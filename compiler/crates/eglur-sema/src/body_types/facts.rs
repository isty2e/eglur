use std::collections::BTreeMap;

use eglur_hir::{ExprId, Mutability};

use super::LocalTypeFacts;

use crate::{
    CallableInstanceId, CallableSignatureId, CanonicalTarget, CheckedNumericOperation,
    ControlRegion, ControlRegionKey, ControlTransfer, ControlTransferSource, DefId, ExpressionKey,
    FactState, FactTable, GenericParameterId, IntegerValue, LocalBindingId, NormalContinuation,
    ReferenceMutability, SemanticFact, SemanticLocation, SubstId, TraitRef, TypeId, WitnessSlotId,
};

/// Stable storage identity retained for a typed place expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypedPlaceKind {
    Local(LocalBindingId),
    StoredField {
        receiver: ExprId,
        field: CanonicalTarget,
    },
    TupleElement {
        receiver: ExprId,
        index: usize,
    },
    Dereference {
        reference: ExprId,
    },
}

/// Checked place classification used by references and later mutation analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedPlace {
    pub(super) kind: TypedPlaceKind,
    pub(super) writable: bool,
}

impl TypedPlace {
    #[must_use]
    pub const fn kind(&self) -> &TypedPlaceKind {
        &self.kind
    }

    #[must_use]
    pub const fn is_writable(&self) -> bool {
        self.writable
    }
}

/// Canonical type and optional storage identity for one expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedExpression {
    pub(super) ty: TypeId,
    pub(super) integer_constant: Option<IntegerValue>,
    pub(super) place: Option<TypedPlace>,
    pub(super) normal_continuation: NormalContinuation,
}

impl TypedExpression {
    /// Exact integer result established without executing source effects.
    #[must_use]
    pub const fn integer_constant(&self) -> Option<IntegerValue> {
        self.integer_constant
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn place(&self) -> Option<&TypedPlace> {
        self.place.as_ref()
    }

    #[must_use]
    pub const fn normal_continuation(&self) -> NormalContinuation {
        self.normal_continuation
    }
}

/// Canonical type and source mutability for one resolved local binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalTypeFact {
    pub(super) ty: TypeId,
    pub(super) mutability: Mutability,
    pub(super) region: ControlRegionKey,
}

impl LocalTypeFact {
    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn mutability(&self) -> Mutability {
        self.mutability
    }

    #[must_use]
    pub const fn region(&self) -> &ControlRegionKey {
        &self.region
    }
}

/// Statically typed ownership operation before path-sensitive flow analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypedOwnershipOperationKind {
    Own,
    Move,
    DropOwn,
}

/// Canonical source binding consumed by one ownership operation occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypedOwnershipOperation {
    pub(super) kind: TypedOwnershipOperationKind,
    pub(super) source: LocalBindingId,
}

impl TypedOwnershipOperation {
    #[must_use]
    pub const fn kind(self) -> TypedOwnershipOperationKind {
        self.kind
    }

    #[must_use]
    pub const fn source(self) -> LocalBindingId {
        self.source
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CarrierArm {
    pub(super) may_select_present: bool,
    pub(super) binding: Option<LocalBindingId>,
}

impl CarrierArm {
    pub(crate) const fn may_select_present(self) -> bool {
        self.may_select_present
    }

    pub(crate) const fn binding(self) -> Option<LocalBindingId> {
        self.binding
    }
}

/// Data-member classification selected from a receiver's canonical nominal type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberKind {
    StoredField { mutability: Mutability },
    Property,
}

/// Canonical data-member lookup result for one member expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemberAccess {
    pub(super) declaration: CanonicalTarget,
    pub(super) kind: MemberKind,
    pub(super) ty: TypeId,
}

impl MemberAccess {
    #[must_use]
    pub const fn declaration(&self) -> &CanonicalTarget {
        &self.declaration
    }

    #[must_use]
    pub const fn kind(&self) -> MemberKind {
        self.kind
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }
}

/// One complete generic callable instantiation, shared across equivalent calls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableInstance {
    pub(super) id: CallableInstanceId,
    pub(super) signature: CallableSignatureId,
    pub(super) substitution: SubstId,
}

impl CallableInstance {
    #[must_use]
    pub const fn id(&self) -> CallableInstanceId {
        self.id
    }

    #[must_use]
    pub const fn signature(&self) -> CallableSignatureId {
        self.signature
    }

    #[must_use]
    pub const fn substitution(&self) -> SubstId {
        self.substitution
    }
}

/// Checked invocation target and arguments in declaration-slot order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvocationTarget {
    Callable(CallableInstanceId),
    CallableValue {
        callee: ExprId,
        ty: TypeId,
    },
    TensorConstructor {
        declaration: CanonicalTarget,
        constructor: crate::TensorSourceConstructor,
        dtype: TypeId,
    },
    Constructor {
        declaration: CanonicalTarget,
        substitution: SubstId,
    },
}

/// Evaluation selected by typing, before a value is bound to a parameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentPassing {
    Value,
    BorrowPlace(ReferenceMutability),
    BorrowTemporary(ReferenceMutability),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvocationArgument {
    expression: ExprId,
    passing: ArgumentPassing,
}

impl InvocationArgument {
    pub(super) const fn new(expression: ExprId, passing: ArgumentPassing) -> Self {
        Self {
            expression,
            passing,
        }
    }

    #[must_use]
    pub const fn expression(self) -> ExprId {
        self.expression
    }

    #[must_use]
    pub const fn passing(self) -> ArgumentPassing {
        self.passing
    }
}

/// Evaluator-facing invocation fact; no lookup or argument rebinding remains.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvocationFact {
    target: InvocationTarget,
    arguments: Box<[InvocationArgument]>,
    evaluation_order: Box<[usize]>,
}

impl InvocationFact {
    pub(super) fn new(
        target: InvocationTarget,
        arguments: impl IntoIterator<Item = (usize, InvocationArgument)>,
    ) -> Self {
        let (source_order, arguments): (Vec<_>, Vec<_>) = arguments.into_iter().unzip();
        let mut evaluation_order = (0..arguments.len()).collect::<Vec<_>>();
        evaluation_order.sort_by_key(|&index| source_order[index]);
        debug_assert!(
            evaluation_order
                .windows(2)
                .all(|pair| source_order[pair[0]] != source_order[pair[1]]),
            "checked invocation slots must have distinct evaluation order"
        );
        Self {
            target,
            arguments: arguments.into_boxed_slice(),
            evaluation_order: evaluation_order.into_boxed_slice(),
        }
    }

    #[must_use]
    pub const fn target(&self) -> &InvocationTarget {
        &self.target
    }

    #[must_use]
    pub fn arguments(&self) -> &[InvocationArgument] {
        &self.arguments
    }

    /// Returns arguments in normative source evaluation order.
    pub fn evaluation_order(&self) -> impl ExactSizeIterator<Item = &InvocationArgument> {
        self.evaluation_order
            .iter()
            .map(|&index| &self.arguments[index])
    }
}

/// First-order trait proof request emitted only after complete local inference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingTraitObligation {
    pub(super) site: ExpressionKey,
    pub(super) owner: DefId,
    pub(super) location: SemanticLocation,
    pub(super) parameter: GenericParameterId,
    pub(super) bound_index: usize,
    pub(super) subject: TypeId,
    pub(super) trait_ref: TraitRef,
    pub(super) destination: Option<WitnessSlotId>,
}

impl PendingTraitObligation {
    #[must_use]
    pub const fn site(&self) -> ExpressionKey {
        self.site
    }

    #[must_use]
    pub const fn owner(&self) -> DefId {
        self.owner
    }

    #[must_use]
    pub const fn location(&self) -> SemanticLocation {
        self.location
    }

    #[must_use]
    pub const fn parameter(&self) -> &GenericParameterId {
        &self.parameter
    }

    #[must_use]
    pub const fn bound_index(&self) -> usize {
        self.bound_index
    }

    #[must_use]
    pub const fn subject(&self) -> TypeId {
        self.subject
    }

    #[must_use]
    pub const fn trait_ref(&self) -> &TraitRef {
        &self.trait_ref
    }

    /// Returns the callee-owned witness slot filled by this obligation, when source-defined.
    #[must_use]
    pub const fn destination(&self) -> Option<WitnessSlotId> {
        self.destination
    }
}

/// Trait-method target selected during body typing and proved after the body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodDispatchSite {
    Invocation(ExpressionKey),
    CallableValue(ExpressionKey),
}

impl MethodDispatchSite {
    #[must_use]
    pub const fn key(self) -> ExpressionKey {
        match self {
            Self::Invocation(key) | Self::CallableValue(key) => key,
        }
    }
}

/// Trait-method target selected during body typing and proved after the body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingMethodDispatch {
    pub(super) site: MethodDispatchSite,
    pub(super) owner: DefId,
    pub(super) location: SemanticLocation,
    pub(super) subject: TypeId,
    pub(super) trait_ref: TraitRef,
    pub(super) method: CanonicalTarget,
}

impl PendingMethodDispatch {
    #[must_use]
    pub const fn site(&self) -> MethodDispatchSite {
        self.site
    }

    #[must_use]
    pub const fn owner(&self) -> DefId {
        self.owner
    }

    #[must_use]
    pub const fn location(&self) -> SemanticLocation {
        self.location
    }

    #[must_use]
    pub const fn subject(&self) -> TypeId {
        self.subject
    }

    #[must_use]
    pub const fn trait_ref(&self) -> &TraitRef {
        &self.trait_ref
    }

    #[must_use]
    pub const fn method(&self) -> &CanonicalTarget {
        &self.method
    }
}

/// Result contract established by checking one source callable or constant body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodyTypeSummary {
    pub(super) result: TypeId,
}

/// Receiver-identity commitment for one contextual `reduce_by` expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReduceByCommitment {
    Ordinary,
    TensorSingle,
    TensorPhases,
}

impl BodyTypeSummary {
    #[must_use]
    pub const fn result(self) -> TypeId {
        self.result
    }
}

/// Body-owned canonical facts produced without trait search or control-flow closure.
pub struct BodyTypes {
    pub(super) expressions: FactTable<ExpressionKey, TypedExpression>,
    pub(super) locals: LocalTypeFacts,
    pub(super) members: FactTable<ExpressionKey, MemberAccess>,
    pub(super) invocations: FactTable<ExpressionKey, InvocationFact>,
    pub(super) callable_values: FactTable<ExpressionKey, CallableInstanceId>,
    pub(super) reduce_by_commitments: FactTable<ExpressionKey, ReduceByCommitment>,
    pub(super) regions: FactTable<ControlRegionKey, ControlRegion>,
    pub(super) checked_numeric: FactTable<ExpressionKey, CheckedNumericOperation>,
    pub(super) ownership_operations: FactTable<ExpressionKey, TypedOwnershipOperation>,
    pub(super) carrier_arms: BTreeMap<ExpressionKey, Box<[CarrierArm]>>,
    pub(super) bodies: FactTable<DefId, BodyTypeSummary>,
    pub(super) transfers: Box<[ControlTransfer]>,
    pub(super) callable_instances: Box<[CallableInstance]>,
    pub(super) obligations: Box<[PendingTraitObligation]>,
    pub(super) method_dispatches: Box<[PendingMethodDispatch]>,
}

impl BodyTypes {
    #[must_use]
    pub fn expression(&self, key: ExpressionKey) -> FactState<'_, TypedExpression> {
        self.expressions.state(&key)
    }

    pub(crate) fn recorded_expression(
        &self,
        key: ExpressionKey,
    ) -> Option<FactState<'_, TypedExpression>> {
        self.expressions.recorded_state(&key)
    }

    #[must_use]
    pub fn local(&self, binding: LocalBindingId) -> FactState<'_, LocalTypeFact> {
        self.locals.state(&binding)
    }

    pub(crate) fn recorded_local(
        &self,
        binding: LocalBindingId,
    ) -> Option<FactState<'_, LocalTypeFact>> {
        self.locals.recorded_state(&binding)
    }

    #[must_use]
    pub fn member(&self, key: ExpressionKey) -> FactState<'_, MemberAccess> {
        self.members.state(&key)
    }

    #[must_use]
    pub fn invocation(&self, key: ExpressionKey) -> FactState<'_, InvocationFact> {
        self.invocations.state(&key)
    }

    #[must_use]
    pub fn callable_value(&self, key: ExpressionKey) -> FactState<'_, CallableInstanceId> {
        self.callable_values.state(&key)
    }

    pub fn callable_values(
        &self,
    ) -> impl ExactSizeIterator<Item = (ExpressionKey, CallableInstanceId)> + '_ {
        self.callable_values.iter().map(|(key, fact)| match fact {
            SemanticFact::Known(instance) => (*key, *instance),
            SemanticFact::Poisoned(_) | SemanticFact::NotApplicable => {
                unreachable!("callable values are recorded only after static selection")
            }
        })
    }

    #[must_use]
    pub fn reduce_by_commitment(&self, key: ExpressionKey) -> FactState<'_, ReduceByCommitment> {
        self.reduce_by_commitments.state(&key)
    }

    pub fn invocations(&self) -> impl ExactSizeIterator<Item = (ExpressionKey, &InvocationFact)> {
        self.invocations.iter().map(|(key, fact)| match fact {
            SemanticFact::Known(invocation) => (*key, invocation),
            SemanticFact::Poisoned(_) | SemanticFact::NotApplicable => {
                unreachable!("invocations are recorded only after successful call typing")
            }
        })
    }

    #[must_use]
    pub fn region(&self, key: &ControlRegionKey) -> FactState<'_, ControlRegion> {
        self.regions.state(key)
    }

    pub fn regions(&self) -> impl ExactSizeIterator<Item = (&ControlRegionKey, &ControlRegion)> {
        self.regions.iter().map(|(key, fact)| match fact {
            SemanticFact::Known(region) => (key, region),
            SemanticFact::Poisoned(_) | SemanticFact::NotApplicable => {
                unreachable!("control regions are structural known facts")
            }
        })
    }

    #[must_use]
    pub fn checked_numeric(&self, key: ExpressionKey) -> FactState<'_, CheckedNumericOperation> {
        self.checked_numeric.state(&key)
    }

    pub fn checked_numeric_operations(
        &self,
    ) -> impl ExactSizeIterator<Item = (ExpressionKey, &CheckedNumericOperation)> {
        self.checked_numeric.iter().map(|(key, fact)| match fact {
            SemanticFact::Known(operation) => (*key, operation),
            SemanticFact::Poisoned(_) | SemanticFact::NotApplicable => {
                unreachable!("checked numeric operations are known after operand typing")
            }
        })
    }

    #[must_use]
    pub fn ownership_operation(
        &self,
        key: ExpressionKey,
    ) -> FactState<'_, TypedOwnershipOperation> {
        self.ownership_operations.state(&key)
    }

    pub(crate) fn carrier_arm(&self, region: &ControlRegionKey) -> Option<CarrierArm> {
        let ControlRegionKey::MatchArm { owner, arm } = region else {
            return None;
        };
        self.carrier_arms.get(owner)?.get(*arm).copied()
    }

    pub fn ownership_operations(
        &self,
    ) -> impl ExactSizeIterator<Item = (ExpressionKey, TypedOwnershipOperation)> + '_ {
        self.ownership_operations
            .iter()
            .map(|(key, fact)| match fact {
                SemanticFact::Known(operation) => (*key, *operation),
                SemanticFact::Poisoned(_) | SemanticFact::NotApplicable => {
                    unreachable!("ownership operations are recorded only after static validation")
                }
            })
    }

    #[must_use]
    pub fn body(&self, declaration: DefId) -> FactState<'_, BodyTypeSummary> {
        self.bodies.state(&declaration)
    }

    #[must_use]
    pub fn transfers(&self) -> &[ControlTransfer] {
        &self.transfers
    }

    #[must_use]
    pub(crate) fn transfers_from(&self, source: &ControlTransferSource) -> &[ControlTransfer] {
        // Completed transfers are published with source as their leading sort key.
        let start = self
            .transfers
            .partition_point(|transfer| transfer.source() < source);
        let remaining = &self.transfers[start..];
        let count = remaining.partition_point(|transfer| transfer.source() == source);
        &remaining[..count]
    }

    #[must_use]
    pub fn callable_instance(&self, id: CallableInstanceId) -> Option<&CallableInstance> {
        self.callable_instances.get(id.index())
    }

    #[must_use]
    pub fn obligations(&self) -> &[PendingTraitObligation] {
        &self.obligations
    }

    #[must_use]
    pub fn method_dispatches(&self) -> &[PendingMethodDispatch] {
        &self.method_dispatches
    }

    #[must_use]
    pub fn fact_count(&self) -> usize {
        self.expressions.iter().len()
            + self.locals.len()
            + self.members.iter().len()
            + self.invocations.iter().len()
            + self.callable_values.iter().len()
            + self.reduce_by_commitments.iter().len()
            + self.regions.iter().len()
            + self.checked_numeric.iter().len()
            + self.ownership_operations.iter().len()
            + self
                .carrier_arms
                .values()
                .map(|arms| arms.len())
                .sum::<usize>()
            + self.bodies.iter().len()
            + self.transfers.len()
            + self.callable_instances.len()
            + self.obligations.len()
            + self.method_dispatches.len()
    }

    pub(crate) fn has_poisoned_facts(&self) -> bool {
        self.expressions.has_poisoned()
            || self.locals.has_poisoned()
            || self.members.has_poisoned()
            || self.invocations.has_poisoned()
            || self.callable_values.has_poisoned()
            || self.reduce_by_commitments.has_poisoned()
            || self.regions.has_poisoned()
            || self.checked_numeric.has_poisoned()
            || self.ownership_operations.has_poisoned()
            || self.bodies.has_poisoned()
    }
}
