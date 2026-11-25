mod flow;
mod traversal;

use flow::FlowState;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use eglur_diagnostics::{DiagnosticStoreError, Poison, PoisonAccumulator};
use eglur_hir::{
    Body, ConstructorPayload, ExprId, ExprKind, FunctionBody, ItemKind, OriginId, PatternId,
    ReduceBySingleValue, StmtId, StmtKind,
};
use eglur_source::FileId;

use crate::resolve::PatternBindingQueryError;
use crate::{
    BodyTypes, CallableSignature, CallableSignatureId, CanonicalTarget, CheckedDispatch,
    CheckedInvocation, DeclarationCollection, DeclarationSignature, DeclarationSignatures, DefId,
    ExpressionKey, FactState, InvocationTarget, LocalBindingId, ModuleId, NameResolution,
    ReduceByCommitment, ReferenceMutability, ResolutionKey, ResolvedName, SemanticSource,
    TraitEvidence, TypeKind, TypeStoreError,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum MutableRoot {
    Local(LocalBindingId),
    Boundary(LocalBindingId),
    Nonlocal,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MutablePath {
    roots: Box<[MutableRoot]>,
}

impl MutablePath {
    fn new(roots: impl IntoIterator<Item = MutableRoot>) -> Self {
        Self {
            roots: roots
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    pub(super) fn roots(&self) -> &[MutableRoot] {
        &self.roots
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum EffectDispatch {
    Target(CanonicalTarget),
    BoundRequirement { method: CanonicalTarget },
    Constructor,
    CallableValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EffectCall {
    origin: OriginId,
    dispatch: EffectDispatch,
    bang: bool,
    mutable_paths: Box<[MutablePath]>,
}

impl EffectCall {
    pub(super) const fn origin(&self) -> OriginId {
        self.origin
    }

    pub(super) const fn dispatch(&self) -> &EffectDispatch {
        &self.dispatch
    }

    pub(super) const fn is_bang(&self) -> bool {
        self.bang
    }

    pub(super) fn mutable_paths(&self) -> &[MutablePath] {
        &self.mutable_paths
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EffectMutation {
    origin: OriginId,
    path: MutablePath,
}

impl EffectMutation {
    pub(super) const fn origin(&self) -> OriginId {
        self.origin
    }

    pub(super) const fn path(&self) -> &MutablePath {
        &self.path
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum EffectOperation {
    Call(EffectCall),
    Mutation(EffectMutation),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EffectBody {
    owner: DefId,
    signature: CallableSignatureId,
    operations: Box<[EffectOperation]>,
}

#[derive(Debug)]
pub(super) enum EffectBodyFact {
    Known(EffectBody),
    Poisoned { owner: DefId, poison: Poison },
    Unperformed { owner: DefId },
}

impl EffectBodyFact {
    pub(super) const fn owner(&self) -> DefId {
        match self {
            Self::Known(body) => body.owner(),
            Self::Poisoned { owner, .. } | Self::Unperformed { owner } => *owner,
        }
    }

    pub(super) const fn known(&self) -> Option<&EffectBody> {
        match self {
            Self::Known(body) => Some(body),
            Self::Poisoned { .. } | Self::Unperformed { .. } => None,
        }
    }
}

impl EffectBody {
    pub(super) const fn owner(&self) -> DefId {
        self.owner
    }

    pub(super) const fn signature(&self) -> CallableSignatureId {
        self.signature
    }

    pub(super) fn operations(&self) -> &[EffectOperation] {
        &self.operations
    }
}

pub(super) fn collect_effect_bodies(
    collection: &DeclarationCollection,
    names: &NameResolution,
    sources: &[SemanticSource<'_>],
    signatures: &DeclarationSignatures,
    body_types: &BodyTypes,
    trait_evidence: &TraitEvidence,
) -> Result<Box<[EffectBodyFact]>, EffectBodyError> {
    let mut source_map = BTreeMap::new();
    for source in sources {
        if source_map.insert(source.file(), source).is_some() {
            return Err(EffectBodyError::DuplicateSource(source.file()));
        }
    }
    let mut bodies = Vec::new();
    for declaration in collection
        .modules()
        .flat_map(|module| module.declarations())
    {
        let FactState::Known(DeclarationSignature::Callable(signature_id)) =
            signatures.source_declaration(declaration.id())
        else {
            continue;
        };
        let signature = signatures
            .callable(*signature_id)
            .ok_or(EffectBodyError::UnknownCallableSignature(*signature_id))?;
        if signature.body().is_none()
            || !matches!(body_types.body(declaration.id()), FactState::Known(_))
        {
            continue;
        }
        let module = collection
            .module(declaration.id().module())
            .ok_or(EffectBodyError::UnknownModule(declaration.id().module()))?;
        let source = source_map
            .get(&module.file())
            .copied()
            .ok_or(EffectBodyError::MissingSource(module.file()))?;
        let hir = source
            .hir()
            .ok_or(EffectBodyError::MissingHir(module.file()))?;
        let item_id = declaration
            .item()
            .ok_or(EffectBodyError::MissingDeclarationItem(declaration.id()))?;
        let ItemKind::Function(function) = hir.item(item_id).kind() else {
            return Err(EffectBodyError::WrongDeclarationKind(declaration.id()));
        };
        let root = match function.body_form {
            FunctionBody::Absent => continue,
            FunctionBody::Expression(root) | FunctionBody::Block(root) => root,
        };
        let body = hir.body(function.body);
        let boundary_parameters =
            boundary_parameter_bindings(module.id(), body, signature, signatures, names)?;
        let mut collector = BodyCollector {
            module: module.id(),
            body,
            names,
            signatures,
            body_types,
            trait_evidence,
            boundary_parameters,
            operations: Vec::new(),
            blocking: EffectBodyState::Unblocked,
        };
        let mut state = FlowState::default();
        collector.visit_expression(root, &mut state)?;
        bodies.push(match collector.blocking {
            EffectBodyState::Poisoned(pending) => EffectBodyFact::Poisoned {
                owner: declaration.id(),
                poison: EffectBodyState::complete_provenance(pending),
            },
            EffectBodyState::Unperformed => EffectBodyFact::Unperformed {
                owner: declaration.id(),
            },
            EffectBodyState::Unblocked => EffectBodyFact::Known(EffectBody {
                owner: declaration.id(),
                signature: *signature_id,
                operations: collector.operations.into_boxed_slice(),
            }),
        });
    }
    bodies.sort_by_key(EffectBodyFact::owner);
    Ok(bodies.into_boxed_slice())
}

fn boundary_parameter_bindings(
    module: ModuleId,
    body: &Body,
    signature: &CallableSignature,
    signatures: &DeclarationSignatures,
    names: &NameResolution,
) -> Result<BTreeSet<LocalBindingId>, EffectBodyError> {
    let mut bindings = BTreeSet::new();
    for parameter in signature.parameters() {
        if !matches!(
            signatures.types().kind(parameter.ty())?,
            TypeKind::Reference {
                mutability: ReferenceMutability::Mutable,
                ..
            }
        ) {
            continue;
        }
        let Some(pattern_id) = parameter.source_pattern() else {
            continue;
        };
        bindings.extend(names.pattern_bindings(module, body, pattern_id)?);
    }
    Ok(bindings)
}

struct BodyCollector<'analysis> {
    module: ModuleId,
    body: &'analysis Body,
    names: &'analysis NameResolution,
    signatures: &'analysis DeclarationSignatures,
    body_types: &'analysis BodyTypes,
    trait_evidence: &'analysis TraitEvidence,
    boundary_parameters: BTreeSet<LocalBindingId>,
    operations: Vec<EffectOperation>,
    blocking: EffectBodyState,
}

enum EffectBodyState {
    Unblocked,
    Poisoned(PoisonAccumulator),
    Unperformed,
}

impl EffectBodyState {
    fn include_poison(&mut self, poison: &Poison) -> Result<(), EffectBodyError> {
        // Foreign provenance clears this state rather than restoring prior roots.
        let mut pending = match std::mem::replace(self, Self::Unblocked) {
            Self::Poisoned(pending) => pending,
            Self::Unblocked | Self::Unperformed => PoisonAccumulator::new(),
        };

        #[cfg(test)]
        poison_tests::observe(poison_tests::Step::Input);

        pending.include(poison)?;
        *self = Self::Poisoned(pending);
        Ok(())
    }

    fn mark_unperformed(&mut self) {
        if matches!(self, Self::Unblocked) {
            *self = Self::Unperformed;
        }
    }

    fn complete_provenance(pending: PoisonAccumulator) -> Poison {
        let poison = pending
            .finish()
            .expect("poisoned effect-body state includes nonempty provenance");

        #[cfg(test)]
        poison_tests::observe(poison_tests::Step::Complete);

        poison
    }
}

impl BodyCollector<'_> {
    fn record_call(
        &mut self,
        expression: ExprId,
        origin: OriginId,
        state: &FlowState,
    ) -> Result<(), EffectBodyError> {
        let key = ExpressionKey::new(self.module, expression);
        let invocation = match self.body_types.invocation(key) {
            FactState::Known(invocation) => invocation,
            FactState::Poisoned(poison) => {
                self.block_on_poison(poison)?;
                return Ok(());
            }
            FactState::Unperformed | FactState::NotApplicable => {
                self.block_on_unperformed();
                return Ok(());
            }
        };
        let checked = match self.trait_evidence.invocation(key) {
            FactState::Known(checked) => checked,
            FactState::Poisoned(poison) => {
                self.block_on_poison(poison)?;
                return Ok(());
            }
            FactState::Unperformed | FactState::NotApplicable => {
                self.block_on_unperformed();
                return Ok(());
            }
        };
        let (dispatch, bang) = match invocation.target() {
            InvocationTarget::TensorConstructor {
                declaration,
                constructor,
                ..
            } => (
                EffectDispatch::Target(declaration.clone()),
                constructor.is_bang(),
            ),
            InvocationTarget::Constructor { .. } => (EffectDispatch::Constructor, false),
            InvocationTarget::Callable(_) => {
                let CheckedInvocation::Static { dispatch, .. } = checked else {
                    return Err(EffectBodyError::CheckedInvocationMismatch(key));
                };
                match dispatch {
                    CheckedDispatch::Direct(target) => {
                        if let Some(adapter) = self.signatures.trait_adapter(target) {
                            (
                                EffectDispatch::BoundRequirement {
                                    method: adapter.method().clone(),
                                },
                                self.target_is_bang(adapter.method())?,
                            )
                        } else {
                            (
                                EffectDispatch::Target(target.clone()),
                                self.target_is_bang(target)?,
                            )
                        }
                    }
                    CheckedDispatch::Bound { method, .. } => (
                        EffectDispatch::BoundRequirement {
                            method: method.clone(),
                        },
                        self.target_is_bang(method)?,
                    ),
                    CheckedDispatch::ImplTemplate { callable, .. } => (
                        EffectDispatch::Target(callable.clone()),
                        self.target_is_bang(callable)?,
                    ),
                }
            }
            InvocationTarget::CallableValue { callee, ty } => {
                if !matches!(checked, CheckedInvocation::Indirect { callee: found, ty: found_ty } if found == callee && found_ty == ty)
                {
                    return Err(EffectBodyError::CheckedInvocationMismatch(key));
                }
                (EffectDispatch::CallableValue, false)
            }
        };
        let mutable_paths = match invocation.target() {
            InvocationTarget::TensorConstructor { .. } => Vec::new(),
            InvocationTarget::Constructor { .. } => Vec::new(),
            InvocationTarget::CallableValue { .. } => Vec::new(),
            InvocationTarget::Callable(instance) => {
                let instance = self
                    .body_types
                    .callable_instance(*instance)
                    .ok_or(EffectBodyError::UnknownCallableInstance(*instance))?;
                let signature = self.signatures.callable(instance.signature()).ok_or(
                    EffectBodyError::UnknownCallableSignature(instance.signature()),
                )?;
                if signature.parameters().len() != invocation.arguments().len() {
                    return Err(EffectBodyError::InvocationArityMismatch(key));
                }
                signature
                    .parameters()
                    .iter()
                    .zip(invocation.arguments())
                    .filter_map(|(parameter, &argument)| {
                        matches!(
                            self.signatures.types().kind(parameter.ty()),
                            Ok(TypeKind::Reference {
                                mutability: ReferenceMutability::Mutable,
                                ..
                            })
                        )
                        .then_some(argument.expression())
                    })
                    .map(|argument| self.expression_roots(argument, state).map(MutablePath::new))
                    .collect::<Result<Vec<_>, _>>()?
            }
        };
        self.operations.push(EffectOperation::Call(EffectCall {
            origin,
            dispatch,
            bang,
            mutable_paths: mutable_paths.into_boxed_slice(),
        }));
        Ok(())
    }

    fn block_on_poison(&mut self, poison: &Poison) -> Result<(), EffectBodyError> {
        self.blocking.include_poison(poison)
    }

    fn block_on_unperformed(&mut self) {
        self.blocking.mark_unperformed();
    }

    fn target_is_bang(&self, target: &CanonicalTarget) -> Result<bool, EffectBodyError> {
        let FactState::Known(DeclarationSignature::Callable(signature)) =
            self.signatures.declaration(target)
        else {
            return Err(EffectBodyError::NonCallableDispatch(target.clone()));
        };
        self.signatures
            .callable(*signature)
            .map(CallableSignature::is_bang)
            .ok_or(EffectBodyError::UnknownCallableSignature(*signature))
    }

    fn bind_aliases(
        &self,
        pattern: PatternId,
        initializer_roots: &BTreeSet<MutableRoot>,
        state: &mut FlowState,
    ) -> Result<(), EffectBodyError> {
        let bindings = self
            .names
            .pattern_bindings(self.module, self.body, pattern)?;
        for binding in bindings {
            if self.binding_is_reference(binding)? {
                let roots = if initializer_roots.is_empty() {
                    BTreeSet::from([MutableRoot::Unknown])
                } else {
                    initializer_roots.clone()
                };
                state.rebind(binding, roots);
            }
        }
        Ok(())
    }

    fn update_reference_alias(
        &self,
        target: ExprId,
        value: ExprId,
        state: &mut FlowState,
    ) -> Result<(), EffectBodyError> {
        let key = ExpressionKey::new(self.module, target);
        let FactState::Known(typed) = self.body_types.expression(key) else {
            return Ok(());
        };
        let Some(crate::TypedPlaceKind::Local(binding)) =
            typed.place().map(crate::TypedPlace::kind)
        else {
            return Ok(());
        };
        if self.binding_is_reference(*binding)? {
            let roots = self.expression_roots(value, state)?;
            state.rebind(
                *binding,
                if roots.is_empty() {
                    BTreeSet::from([MutableRoot::Unknown])
                } else {
                    roots
                },
            );
        }
        Ok(())
    }

    fn binding_is_reference(&self, binding: LocalBindingId) -> Result<bool, EffectBodyError> {
        let FactState::Known(local) = self.body_types.local(binding) else {
            return Err(EffectBodyError::MissingLocalType(binding));
        };
        Ok(matches!(
            self.signatures.types().kind(local.ty())?,
            TypeKind::Reference { .. }
        ))
    }

    fn assignment_roots(
        &self,
        target: ExprId,
        state: &FlowState,
    ) -> Result<BTreeSet<MutableRoot>, EffectBodyError> {
        let key = ExpressionKey::new(self.module, target);
        if let FactState::Known(typed) = self.body_types.expression(key)
            && let Some(crate::TypedPlaceKind::Local(binding)) =
                typed.place().map(crate::TypedPlace::kind)
        {
            return Ok(BTreeSet::from([
                state.account_for_escape(MutableRoot::Local(*binding))
            ]));
        }
        self.expression_roots(target, state)
    }

    fn expression_roots(
        &self,
        expression: ExprId,
        state: &FlowState,
    ) -> Result<BTreeSet<MutableRoot>, EffectBodyError> {
        let mut pending = vec![expression];
        let mut roots = BTreeSet::new();
        while let Some(id) = pending.pop() {
            let key = ExpressionKey::new(self.module, id);
            if let FactState::Known(typed) = self.body_types.expression(key)
                && let Some(place) = typed.place()
            {
                match place.kind() {
                    crate::TypedPlaceKind::Local(binding) => {
                        if self.boundary_parameters.contains(binding) {
                            roots.insert(MutableRoot::Boundary(*binding));
                        } else if let Some(aliases) = state.alias_roots(*binding) {
                            roots.extend(aliases.iter().cloned());
                        } else {
                            roots.insert(MutableRoot::Local(*binding));
                        }
                    }
                    crate::TypedPlaceKind::StoredField { receiver, .. }
                    | crate::TypedPlaceKind::TupleElement { receiver, .. } => {
                        pending.push(*receiver)
                    }
                    crate::TypedPlaceKind::Dereference { reference } => pending.push(*reference),
                }
                continue;
            }
            let expression = self
                .body
                .expression(id)
                .ok_or(EffectBodyError::WrongBodyExpression(id))?;
            match expression.kind() {
                ExprKind::Reference { value, .. }
                | ExprKind::Try(value)
                | ExprKind::Group(value)
                | ExprKind::Unsafe(value)
                | ExprKind::Result(value)
                | ExprKind::Checked(value) => pending.push(*value),
                ExprKind::Member { receiver, .. } => {
                    if !matches!(
                        self.names.fact(ResolutionKey::Expression {
                            module: self.module,
                            expression: id,
                        }),
                        FactState::Known(ResolvedName::TypeMemberOwner(_))
                    ) {
                        pending.push(*receiver);
                    }
                }
                ExprKind::TupleProjection { receiver, .. } => pending.push(*receiver),
                ExprKind::Tuple(values) | ExprKind::Array(values) => {
                    pending.extend(values.iter().rev().copied())
                }
                ExprKind::Map(entries) => {
                    for entry in entries.iter().rev() {
                        pending.push(entry.value);
                        pending.push(entry.key);
                    }
                }
                ExprKind::StructLiteral { fields, .. } => {
                    pending.extend(fields.iter().rev().map(|field| field.value))
                }
                ExprKind::VariantConstructor { payload, .. } => match payload {
                    ConstructorPayload::Absent => {}
                    ConstructorPayload::Tuple(values) => {
                        pending.extend(values.iter().rev().copied())
                    }
                    ConstructorPayload::Named(fields) => {
                        pending.extend(fields.iter().rev().map(|field| field.value))
                    }
                },
                ExprKind::If(value) => {
                    pending.extend(
                        [value.then_branch]
                            .into_iter()
                            .chain(
                                [value.else_branch, value.unknown_branch]
                                    .into_iter()
                                    .flatten(),
                            )
                            .rev(),
                    );
                }
                ExprKind::Match(value) => {
                    pending.extend(value.arms.iter().rev().map(|arm| arm.value))
                }
                _ => {
                    if let FactState::Known(typed) = self.body_types.expression(key)
                        && matches!(
                            self.signatures.types().kind(typed.ty()),
                            Ok(TypeKind::Reference { .. })
                        )
                    {
                        roots.insert(MutableRoot::Unknown);
                    }
                }
            }
        }
        // Escape projection is idempotent and this query does not mutate state.
        Ok(roots
            .into_iter()
            .map(|root| state.account_for_escape(root))
            .collect())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum EffectBodyError {
    DuplicateSource(FileId),
    UnknownModule(ModuleId),
    MissingSource(FileId),
    MissingHir(FileId),
    MissingDeclarationItem(DefId),
    WrongDeclarationKind(DefId),
    WrongBodyExpression(ExprId),
    WrongBodyStatement(StmtId),
    PatternBinding(PatternBindingQueryError),
    MissingLocalType(LocalBindingId),
    UnknownCallableInstance(crate::CallableInstanceId),
    UnknownCallableSignature(CallableSignatureId),
    InvocationArityMismatch(ExpressionKey),
    CheckedInvocationMismatch(ExpressionKey),
    NonCallableDispatch(CanonicalTarget),
    DiagnosticStore(DiagnosticStoreError),
    TypeStore(TypeStoreError),
}

impl fmt::Display for EffectBodyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "effect body construction invariant failed: {self:?}"
        )
    }
}

impl std::error::Error for EffectBodyError {}

impl From<TypeStoreError> for EffectBodyError {
    fn from(error: TypeStoreError) -> Self {
        Self::TypeStore(error)
    }
}

impl From<PatternBindingQueryError> for EffectBodyError {
    fn from(error: PatternBindingQueryError) -> Self {
        Self::PatternBinding(error)
    }
}

impl From<DiagnosticStoreError> for EffectBodyError {
    fn from(error: DiagnosticStoreError) -> Self {
        Self::DiagnosticStore(error)
    }
}

#[cfg(test)]
mod poison_tests;
