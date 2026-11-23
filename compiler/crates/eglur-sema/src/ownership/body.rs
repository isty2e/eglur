mod branches;
mod traversal;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;

use eglur_hir::{
    Body, ConstructorPayload, ExprId, ExprKind, OriginId, PatternId, PatternKind,
    ReduceBySingleValue, StmtId, StmtKind,
};

use crate::body_types::CarrierArm;
use crate::{
    BodyTypes, ControlRegionKey, ControlTransfer, ControlTransferSource, DeclarationSignatures,
    DefId, ExpressionKey, FactState, IfBranch, LocalBindingId, ModuleId, NameResolution,
    NormalContinuation, ReduceByCommitment, ResolutionKey, ResolvedName, SemanticFact,
    StatementKey, TypeKind, TypedOwnershipOperationKind, TypedPlaceKind,
};

use super::analysis::OwnershipAnalysisError;
use super::model::OwnershipCause;
use super::{
    OwnershipAvailability, OwnershipBodyStatus, OwnershipBodySummary, OwnershipFailure,
    OwnershipFailureKind, OwnershipRootFact,
};
use branches::{Alternatives, FlowState, RecoverableExits};
use eglur_diagnostics::{Poison, PoisonAccumulator};

pub(super) struct OwnershipBodyAnalysis {
    pub(super) fact: SemanticFact<OwnershipBodySummary>,
    pub(super) findings: OwnershipFindings,
}

#[derive(Clone, Copy)]
pub(super) struct CoverageRecovery<'a> {
    cause: &'a Poison,
    sites: &'a BTreeMap<ExprId, Poison>,
    extent: RecoveryExtent,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RecoveryExtent {
    Body,
    WrittenMatches,
}

impl<'a> CoverageRecovery<'a> {
    pub(super) fn new(
        cause: &'a Poison,
        sites: &'a BTreeMap<ExprId, Poison>,
    ) -> Result<Self, OwnershipAnalysisError> {
        let mut coverage = PoisonAccumulator::default();
        for site in sites.values() {
            coverage
                .include(site)
                .map_err(OwnershipAnalysisError::DiagnosticStore)?;
        }
        let extent = if coverage.finish().is_some_and(|all| all.same_roots(cause)) {
            RecoveryExtent::Body
        } else {
            RecoveryExtent::WrittenMatches
        };
        Ok(Self {
            cause,
            sites,
            extent,
        })
    }
}

#[derive(Default)]
pub(super) struct OwnershipFindings {
    pub(super) failures: Vec<OwnershipFailure>,
    pub(super) superseded_coverage: Vec<Poison>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AvailabilityFact {
    state: OwnershipAvailability,
    first_consume: Option<OriginId>,
}

impl AvailabilityFact {
    const fn owned() -> Self {
        Self {
            state: OwnershipAvailability::Owned,
            first_consume: None,
        }
    }

    fn join(self, other: Self) -> Self {
        Self {
            state: self.state.join(other.state),
            first_consume: match (self.first_consume, other.first_consume) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            },
        }
    }
}

impl Default for AvailabilityFact {
    fn default() -> Self {
        Self::owned()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ValueProvenance {
    roots_by_tuple_path: BTreeMap<Vec<usize>, BTreeSet<LocalBindingId>>,
}

impl ValueProvenance {
    fn is_empty(&self) -> bool {
        self.roots_by_tuple_path.is_empty()
    }

    fn direct(roots: BTreeSet<LocalBindingId>) -> Self {
        let mut provenance = Self::default();
        if !roots.is_empty() {
            provenance.roots_by_tuple_path.insert(Vec::new(), roots);
        }
        provenance
    }

    fn extend(&mut self, other: Self) {
        for (path, roots) in other.roots_by_tuple_path {
            self.roots_by_tuple_path
                .entry(path)
                .or_default()
                .extend(roots);
        }
    }

    fn prefixed(self, index: usize) -> Self {
        let roots_by_tuple_path = self
            .roots_by_tuple_path
            .into_iter()
            .map(|(path, roots)| {
                let mut prefixed = Vec::with_capacity(path.len() + 1);
                prefixed.push(index);
                prefixed.extend(path);
                (prefixed, roots)
            })
            .collect();
        Self {
            roots_by_tuple_path,
        }
    }

    fn projected(&self, index: usize) -> Self {
        let lower = [index];
        let upper = index.checked_add(1).map(|next| [next]);
        let bounds = (
            Bound::Included(lower.as_slice()),
            upper
                .as_ref()
                .map_or(Bound::Unbounded, |upper| Bound::Excluded(upper.as_slice())),
        );
        let mut projected = Self::default();
        for (path, roots) in self.roots_by_tuple_path.range::<[usize], _>(bounds) {
            projected
                .roots_by_tuple_path
                .entry(path[1..].to_vec())
                .or_default()
                .extend(roots);
        }
        projected
    }

    fn roots(&self) -> BTreeSet<LocalBindingId> {
        self.roots_by_tuple_path
            .values()
            .flatten()
            .copied()
            .collect()
    }
}

#[derive(Clone, Debug, Default)]
struct ExpressionFlow {
    continuing: bool,
    provenance: ValueProvenance,
}

impl ExpressionFlow {
    fn continuing() -> Self {
        Self {
            continuing: true,
            provenance: ValueProvenance::default(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn analyze_body(
    owner: DefId,
    module: ModuleId,
    body: &Body,
    root: ExprId,
    parameters: BTreeSet<LocalBindingId>,
    names: &NameResolution,
    signatures: &DeclarationSignatures,
    body_types: &BodyTypes,
    recovery: Option<CoverageRecovery<'_>>,
) -> Result<OwnershipBodyAnalysis, OwnershipAnalysisError> {
    let mut analyzer = BodyAnalyzer {
        owner,
        module,
        body,
        names,
        signatures,
        body_types,
        recovery,
        superseded_coverage: Vec::new(),
        roots: BTreeMap::new(),
        locals_by_region: BTreeMap::new(),
        failures: Vec::new(),
        operation_count: 0,
        unperformed: false,
        recoverable_exits: Vec::new(),
        tentative: BTreeSet::new(),
        pending_payloads: Vec::new(),
    };
    if let Some(CoverageRecovery {
        extent: RecoveryExtent::WrittenMatches,
        sites,
        ..
    }) = recovery
    {
        // Other ill-typed expressions cannot establish the state entering these matches.
        for &site in sites.keys() {
            analyzer.visit_expression(site, &mut FlowState::written_only(), None)?;
        }
    } else {
        analyzer.visit_callable(root, parameters)?;
    }
    if let Some(recovery) = recovery {
        return Ok(OwnershipBodyAnalysis {
            fact: SemanticFact::Poisoned(recovery.cause.clone()),
            findings: if analyzer.unperformed {
                OwnershipFindings::default()
            } else {
                OwnershipFindings {
                    failures: analyzer.failures,
                    superseded_coverage: analyzer.superseded_coverage,
                }
            },
        });
    }
    let ownership_local_exists = body.patterns().try_fold(
        false,
        |found, (pattern_id, pattern)| -> Result<_, OwnershipAnalysisError> {
            if found {
                return Ok(true);
            }
            match pattern.kind() {
                PatternKind::Binding(_) | PatternKind::Alias { .. } => {}
                PatternKind::Wildcard
                | PatternKind::Literal(_)
                | PatternKind::Unit
                | PatternKind::Group(_)
                | PatternKind::Or(_)
                | PatternKind::Tuple(_)
                | PatternKind::Variant { .. }
                | PatternKind::Struct { .. }
                | PatternKind::Invalid => return Ok(false),
            }
            let Some(binding) = names.body_bindings(module, body.id()).and_then(|bindings| {
                bindings.binding(crate::resolve::PatternBindingSite::Name(pattern_id))
            }) else {
                return Ok(true);
            };
            match body_types.recorded_local(binding) {
                Some(FactState::Known(local)) => {
                    Ok(signatures.types().contains_ownership(local.ty())?)
                }
                Some(FactState::NotApplicable) => Ok(false),
                Some(FactState::Poisoned(_) | FactState::Unperformed) | None => Ok(true),
            }
        },
    )?;
    let ownership_expression_exists = body.expressions().try_fold(
        false,
        |found, (expression, _)| -> Result<_, OwnershipAnalysisError> {
            if found {
                return Ok(true);
            }
            match body_types.recorded_expression(ExpressionKey::new(module, expression)) {
                Some(FactState::Known(expression)) => {
                    Ok(signatures.types().contains_ownership(expression.ty())?)
                }
                Some(FactState::Poisoned(_) | FactState::Unperformed) => Ok(true),
                Some(FactState::NotApplicable) | None => Ok(false),
            }
        },
    )?;
    let ownership_relevant =
        ownership_local_exists || ownership_expression_exists || analyzer.operation_count != 0;
    let unperformed = analyzer.unperformed && ownership_relevant;
    let roots = analyzer
        .roots
        .into_iter()
        .map(|(binding, region)| OwnershipRootFact::new(binding, region));
    Ok(OwnershipBodyAnalysis {
        fact: SemanticFact::Known(OwnershipBodySummary::new(
            if unperformed {
                OwnershipBodyStatus::Unperformed
            } else {
                OwnershipBodyStatus::Checked
            },
            roots,
            analyzer.operation_count,
        )),
        findings: if unperformed {
            OwnershipFindings::default()
        } else {
            OwnershipFindings {
                failures: analyzer.failures,
                superseded_coverage: analyzer.superseded_coverage,
            }
        },
    })
}

struct BodyAnalyzer<'analysis> {
    owner: DefId,
    module: ModuleId,
    body: &'analysis Body,
    names: &'analysis NameResolution,
    signatures: &'analysis DeclarationSignatures,
    body_types: &'analysis BodyTypes,
    recovery: Option<CoverageRecovery<'analysis>>,
    superseded_coverage: Vec<Poison>,
    roots: BTreeMap<LocalBindingId, ControlRegionKey>,
    locals_by_region: BTreeMap<ControlRegionKey, BTreeSet<LocalBindingId>>,
    failures: Vec<OwnershipFailure>,
    operation_count: usize,
    unperformed: bool,
    recoverable_exits: Vec<(ControlRegionKey, RecoverableExits)>,
    tentative: BTreeSet<LocalBindingId>,
    pending_payloads: Vec<(ControlRegionKey, OriginId)>,
}

impl BodyAnalyzer<'_> {
    fn visit_callable(
        &mut self,
        root: ExprId,
        parameters: BTreeSet<LocalBindingId>,
    ) -> Result<(), OwnershipAnalysisError> {
        let mut state = FlowState::default();
        for parameter in parameters {
            self.introduce_root(parameter, &mut state)?;
        }
        let region = ControlRegionKey::Callable(self.owner);
        let flow = self.visit_expression(root, &mut state, Some(region.clone()))?;
        let expression = self
            .body
            .expression(root)
            .ok_or(OwnershipAnalysisError::WrongBodyExpression(root))?;
        if !matches!(expression.kind(), ExprKind::Block { .. }) {
            self.recover_missing_move(root, &mut state)?;
            if flow.continuing {
                self.check_transfer_source(
                    &ControlTransferSource::RegionCompletion(region),
                    expression.origin(),
                    false,
                    &mut state,
                );
            }
        }
        Ok(())
    }

    fn poisoned_value(&self, key: ExpressionKey) -> bool {
        self.recovery.is_some() && matches!(self.body_types.expression(key), FactState::Poisoned(_))
    }

    fn visit_ownership_operation(
        &mut self,
        key: ExpressionKey,
        origin: OriginId,
        state: &mut FlowState,
    ) -> Result<ExpressionFlow, OwnershipAnalysisError> {
        let FactState::Known(operation) = self.body_types.ownership_operation(key) else {
            if self.poisoned_value(key) {
                return Ok(ExpressionFlow::default());
            }
            return Err(OwnershipAnalysisError::MissingExpressionType(key));
        };
        self.operation_count += 1;
        match operation.kind() {
            TypedOwnershipOperationKind::Own => {
                self.consume_source(operation.source(), origin, state);
            }
            TypedOwnershipOperationKind::Move | TypedOwnershipOperationKind::DropOwn => {
                if !self.reject_tentative_use(operation.source(), origin) {
                    self.consume_root(operation.source(), origin, state);
                }
            }
        }
        let continuing = !matches!(
            operation.kind(),
            TypedOwnershipOperationKind::Own
                | TypedOwnershipOperationKind::Move
                | TypedOwnershipOperationKind::DropOwn
        ) || self.expression_continues(key, true);
        Ok(ExpressionFlow {
            continuing,
            provenance: ValueProvenance::default(),
        })
    }

    fn visit_name(
        &mut self,
        expression: ExprId,
        origin: OriginId,
        state: &FlowState,
    ) -> Result<ExpressionFlow, OwnershipAnalysisError> {
        let Some(binding) = self.expression_local(expression)? else {
            return Ok(ExpressionFlow::continuing());
        };
        self.reject_tentative_use(binding, origin);
        self.use_binding(binding, origin, state);
        let provenance = if self.binding_contains_reference(binding)? {
            state.alias(binding).cloned().unwrap_or_default()
        } else {
            ValueProvenance::default()
        };
        Ok(ExpressionFlow {
            continuing: true,
            provenance,
        })
    }

    fn reject_tentative_use(&mut self, binding: LocalBindingId, origin: OriginId) -> bool {
        if !self.tentative.contains(&binding) {
            return false;
        }
        self.failures.push(OwnershipFailure::new(
            self.owner,
            OwnershipFailureKind::GuardConsume,
            origin,
            binding,
            None,
        ));
        true
    }

    fn retain_in_flight(
        &self,
        expression: ExprId,
        provenance: &ValueProvenance,
        in_flight: &mut Vec<(OriginId, BTreeSet<LocalBindingId>)>,
    ) -> Result<(), OwnershipAnalysisError> {
        let roots = provenance.roots();
        if !roots.is_empty() {
            in_flight.push((self.expression_origin(expression)?, roots));
        }
        Ok(())
    }

    fn check_in_flight(
        &mut self,
        in_flight: &[(OriginId, BTreeSet<LocalBindingId>)],
        state: &FlowState,
    ) {
        if !state.has_input() {
            return;
        }
        for (origin, roots) in in_flight {
            for &root in roots {
                let fact = state.availability(root);
                if fact.state != OwnershipAvailability::Owned {
                    self.failures.push(OwnershipFailure::new(
                        self.owner,
                        OwnershipFailureKind::UseAfterDischarge,
                        *origin,
                        root,
                        fact.first_consume,
                    ));
                }
            }
        }
    }

    fn record_local_region(&mut self, binding: LocalBindingId) {
        if let FactState::Known(local) = self.body_types.local(binding) {
            self.locals_by_region
                .entry(local.region().clone())
                .or_default()
                .insert(binding);
        }
    }

    fn bind_let_pattern(
        &mut self,
        pattern: PatternId,
        initializer: ExprId,
        flow: &ExpressionFlow,
        state: &mut FlowState,
    ) -> Result<(), OwnershipAnalysisError> {
        let bindings = self
            .names
            .pattern_bindings(self.module, self.body, pattern)?;
        let mut recovered_transfer = false;
        for binding in bindings {
            if self.binding_is_root(binding)? {
                if !recovered_transfer {
                    self.recover_missing_move(initializer, state)?;
                    recovered_transfer = true;
                }
                self.introduce_root(binding, state)?;
            }
        }
        self.bind_pattern_provenance(pattern, &flow.provenance, state)
    }

    fn bind_pattern_provenance(
        &mut self,
        pattern: PatternId,
        subject: &ValueProvenance,
        state: &mut FlowState,
    ) -> Result<(), OwnershipAnalysisError> {
        use std::borrow::Cow;

        if !state.has_input() || subject.is_empty() {
            return Ok(());
        }
        let mut pending = vec![(pattern, Cow::Borrowed(subject))];
        while let Some((id, provenance)) = pending.pop() {
            let pattern = self
                .body
                .pattern(id)
                .ok_or(OwnershipAnalysisError::WrongBodyPattern(id))?;
            match pattern.kind() {
                PatternKind::Binding(name) | PatternKind::Alias { name, .. } => {
                    let binding = self
                        .names
                        .body_bindings(self.module, self.body.id())
                        .and_then(|bindings| {
                            bindings.binding(crate::resolve::PatternBindingSite::Name(id))
                        })
                        .ok_or(OwnershipAnalysisError::MissingLocalOrigin {
                            module: self.module,
                            body: self.body.id(),
                            origin: name.origin(),
                        })?;
                    let holds_reference = self.binding_contains_reference(binding)?;
                    if holds_reference {
                        self.record_local_region(binding);
                    }
                    if let PatternKind::Alias { pattern, .. } = pattern.kind() {
                        if holds_reference {
                            state.set_alias(binding, provenance.as_ref().clone());
                        }
                        pending.push((*pattern, provenance));
                    } else if holds_reference {
                        state.set_alias(binding, provenance.into_owned());
                    }
                }
                PatternKind::Group(inner) => pending.push((*inner, provenance)),
                PatternKind::Tuple(elements) => {
                    for (index, element) in elements.iter().enumerate().rev() {
                        if let eglur_hir::PatternElement::Pattern(inner) = element {
                            let projected = provenance.projected(index);
                            if !projected.is_empty() {
                                pending.push((*inner, Cow::Owned(projected)));
                            }
                        }
                    }
                }
                PatternKind::Wildcard | PatternKind::Unit | PatternKind::Literal(_) => {}
                PatternKind::Or(_)
                | PatternKind::Struct { .. }
                | PatternKind::Variant { .. }
                | PatternKind::Invalid => self.unperformed = true,
            }
        }
        Ok(())
    }

    fn introduce_root(
        &mut self,
        binding: LocalBindingId,
        state: &mut FlowState,
    ) -> Result<(), OwnershipAnalysisError> {
        if !state.has_input() || !self.binding_is_root(binding)? {
            return Ok(());
        }
        let FactState::Known(local) = self.body_types.local(binding) else {
            return Err(OwnershipAnalysisError::MissingLocalType(binding));
        };
        self.record_local_region(binding);
        self.roots
            .entry(binding)
            .or_insert_with(|| local.region().clone());
        state.set_availability(binding, AvailabilityFact::owned());
        Ok(())
    }

    fn consume_root(&mut self, binding: LocalBindingId, origin: OriginId, state: &mut FlowState) {
        if !state.has_input() {
            return;
        }
        self.record_local_region(binding);
        let mut fact = state.availability(binding);
        match fact.state {
            OwnershipAvailability::Owned => {
                fact.state = OwnershipAvailability::Consumed;
                fact.first_consume = Some(origin);
            }
            OwnershipAvailability::Consumed | OwnershipAvailability::MaybeConsumed => {
                let related = fact.first_consume;
                fact.state = OwnershipAvailability::Consumed;
                self.failures.push(OwnershipFailure::new(
                    self.owner,
                    OwnershipFailureKind::DoubleMove,
                    origin,
                    binding,
                    related,
                ));
            }
        }
        state.set_availability(binding, fact);
    }

    fn consume_source(&mut self, binding: LocalBindingId, origin: OriginId, state: &mut FlowState) {
        if !state.has_input() {
            return;
        }
        self.record_local_region(binding);
        let mut fact = state.availability(binding);
        if fact.state == OwnershipAvailability::Owned {
            fact.state = OwnershipAvailability::Consumed;
            fact.first_consume = Some(origin);
            state.set_availability(binding, fact);
            return;
        }
        self.failures.push(OwnershipFailure::new(
            self.owner,
            OwnershipFailureKind::UseAfterDischarge,
            origin,
            binding,
            fact.first_consume,
        ));
        fact.state = OwnershipAvailability::Consumed;
        state.set_availability(binding, fact);
    }

    fn use_binding(&mut self, binding: LocalBindingId, origin: OriginId, state: &FlowState) {
        if !state.has_input() {
            return;
        }
        let fact = state.availability(binding);
        if fact.state != OwnershipAvailability::Owned {
            self.failures.push(OwnershipFailure::new(
                self.owner,
                OwnershipFailureKind::UseAfterDischarge,
                origin,
                binding,
                fact.first_consume,
            ));
        }
        if let Some(provenance) = state.alias(binding) {
            self.use_roots(&provenance.roots(), origin, state);
        }
    }

    fn use_roots(&mut self, roots: &BTreeSet<LocalBindingId>, origin: OriginId, state: &FlowState) {
        if !state.has_input() {
            return;
        }
        for &root in roots {
            let fact = state.availability(root);
            if fact.state != OwnershipAvailability::Owned {
                self.failures.push(OwnershipFailure::new(
                    self.owner,
                    OwnershipFailureKind::UseAfterDischarge,
                    origin,
                    root,
                    fact.first_consume,
                ));
            }
        }
    }

    fn recover_missing_move(
        &mut self,
        expression: ExprId,
        state: &mut FlowState,
    ) -> Result<(), OwnershipAnalysisError> {
        if self
            .recovery
            .is_some_and(|recovery| recovery.extent == RecoveryExtent::WrittenMatches)
        {
            return Ok(());
        }
        let mut pending = vec![expression];
        while let Some(id) = pending.pop() {
            if self.poisoned_value(ExpressionKey::new(self.module, id)) {
                continue;
            }
            let expression = self
                .body
                .expression(id)
                .ok_or(OwnershipAnalysisError::WrongBodyExpression(id))?;
            match expression.kind() {
                ExprKind::Group(inner) => pending.push(*inner),
                ExprKind::If(value) => {
                    pending.extend(
                        [
                            Some(value.then_branch),
                            value.else_branch,
                            value.unknown_branch,
                        ]
                        .into_iter()
                        .flatten()
                        .rev(),
                    );
                }
                ExprKind::Match(value) => {
                    pending.extend(value.arms.iter().rev().map(|arm| arm.value));
                }
                ExprKind::Name(_) => {
                    if let Some(binding) = self.named_root(id)? {
                        let origin = expression.origin();
                        if self.tentative.contains(&binding) {
                            // The value occurrence already reported the guard-specific diagnostic.
                            continue;
                        }
                        self.failures.push(OwnershipFailure::new(
                            self.owner,
                            OwnershipFailureKind::TransferRequiresMove,
                            origin,
                            binding,
                            None,
                        ));
                        self.recover_consumed(binding, origin, state);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn recover_consumed(
        &mut self,
        binding: LocalBindingId,
        origin: OriginId,
        state: &mut FlowState,
    ) {
        if !state.has_input() {
            return;
        }
        self.record_local_region(binding);
        let mut fact = state.availability(binding);
        if fact.state == OwnershipAvailability::Owned {
            fact.first_consume = Some(origin);
        }
        fact.state = OwnershipAvailability::Consumed;
        state.set_availability(binding, fact);
    }

    fn named_root(
        &self,
        mut expression: ExprId,
    ) -> Result<Option<LocalBindingId>, OwnershipAnalysisError> {
        loop {
            let value = self
                .body
                .expression(expression)
                .ok_or(OwnershipAnalysisError::WrongBodyExpression(expression))?;
            if let ExprKind::Group(inner) = value.kind() {
                expression = *inner;
                continue;
            }
            if !matches!(value.kind(), ExprKind::Name(_)) {
                return Ok(None);
            }
            let Some(binding) = self.expression_local(expression)? else {
                return Ok(None);
            };
            return self
                .binding_is_root(binding)
                .map(|is_root| is_root.then_some(binding));
        }
    }

    fn tentative_place(
        &self,
        expression: ExprId,
    ) -> Result<Option<LocalBindingId>, OwnershipAnalysisError> {
        if self.tentative.is_empty() {
            return Ok(None);
        }
        Ok(self
            .named_root(expression)?
            .filter(|id| self.tentative.contains(id)))
    }

    fn check_side_exits(&mut self, key: ExpressionKey, origin: OriginId, state: &FlowState) {
        if !state.has_input() {
            return;
        }
        let source = ControlTransferSource::Expression(key);
        let body_types = self.body_types;
        for transfer in body_types.transfers_from(&source) {
            let leaving = self.check_exit_obligations(transfer, origin, false, state);
            if let Some((_, exits)) = self
                .recoverable_exits
                .iter_mut()
                .rev()
                .find(|(region, _)| region == transfer.target())
            {
                let mut exit_state = state.exit_snapshot();
                for id in leaving {
                    exit_state.end_binding(id);
                }
                exits.include(exit_state);
            }
        }
    }

    fn check_transfer_source(
        &mut self,
        source: &ControlTransferSource,
        origin: OriginId,
        closing_delimiter: bool,
        state: &mut FlowState,
    ) {
        if !state.has_input() {
            return;
        }
        let body_types = self.body_types;
        for transfer in body_types.transfers_from(source) {
            for binding in self.check_exit_obligations(transfer, origin, closing_delimiter, state) {
                state.end_binding(binding);
            }
        }
    }

    fn check_exit_obligations(
        &mut self,
        transfer: &ControlTransfer,
        origin: OriginId,
        closing_delimiter: bool,
        state: &FlowState,
    ) -> BTreeSet<LocalBindingId> {
        // Region order must not replace binding order when several regions exit.
        let mut bindings = BTreeSet::new();
        let pending = self
            .pending_payloads
            .last()
            .filter(|_| !matches!(transfer.kind(), crate::ControlTransferKind::MatchGuardFalse));
        for region in transfer.exited_regions() {
            if let Some(locals) = self.locals_by_region.get(region) {
                bindings.extend(locals.iter().copied());
            }
            // An edge leaving any pending payload necessarily leaves the innermost one.
            if let Some((guard, scrutinee)) = pending
                && guard == region
            {
                self.failures.push(OwnershipFailure::at(
                    self.owner,
                    origin,
                    OwnershipCause::PendingMatchExit {
                        scrutinee: *scrutinee,
                    },
                ));
            }
        }
        for &binding in &bindings {
            let fact = state.availability(binding);
            if self.roots.contains_key(&binding) && fact.state != OwnershipAvailability::Consumed {
                let failure = OwnershipFailure::new(
                    self.owner,
                    OwnershipFailureKind::PathUnbalanced,
                    origin,
                    binding,
                    fact.first_consume,
                );
                self.failures.push(if closing_delimiter {
                    failure.at_closing_delimiter()
                } else {
                    failure
                });
                // All failures on this edge have the same span and code.
                // Binding order selects the first witness.
                break;
            }
        }
        bindings
    }

    fn expression_roots(
        &self,
        mut expression: ExprId,
        state: &FlowState,
    ) -> Result<BTreeSet<LocalBindingId>, OwnershipAnalysisError> {
        loop {
            let key = ExpressionKey::new(self.module, expression);
            let FactState::Known(typed) = self.body_types.expression(key) else {
                return Ok(BTreeSet::new());
            };
            let Some(place) = typed.place() else {
                return Ok(BTreeSet::new());
            };
            match place.kind() {
                TypedPlaceKind::Local(binding) if self.binding_is_reference(*binding)? => {
                    return Ok(state
                        .alias(*binding)
                        .map(ValueProvenance::roots)
                        .unwrap_or_default());
                }
                TypedPlaceKind::Local(binding) => return Ok(BTreeSet::from([*binding])),
                TypedPlaceKind::StoredField { receiver, .. }
                | TypedPlaceKind::TupleElement { receiver, .. } => expression = *receiver,
                TypedPlaceKind::Dereference { reference } => expression = *reference,
            }
        }
    }

    fn expression_local(
        &self,
        expression: ExprId,
    ) -> Result<Option<LocalBindingId>, OwnershipAnalysisError> {
        let key = ExpressionKey::new(self.module, expression);
        let FactState::Known(typed) = self.body_types.expression(key) else {
            return Ok(None);
        };
        Ok(match typed.place().map(crate::TypedPlace::kind) {
            Some(TypedPlaceKind::Local(binding)) => Some(*binding),
            _ => None,
        })
    }

    fn expression_continues(&self, key: ExpressionKey, fallback: bool) -> bool {
        match self.body_types.expression(key) {
            FactState::Known(typed) => typed.normal_continuation() == NormalContinuation::Present,
            FactState::Poisoned(_) | FactState::Unperformed | FactState::NotApplicable => fallback,
        }
    }

    fn expression_origin(&self, expression: ExprId) -> Result<OriginId, OwnershipAnalysisError> {
        self.body
            .expression(expression)
            .map(eglur_hir::Expr::origin)
            .ok_or(OwnershipAnalysisError::WrongBodyExpression(expression))
    }

    fn binding_is_root(&self, binding: LocalBindingId) -> Result<bool, OwnershipAnalysisError> {
        let FactState::Known(local) = self.body_types.local(binding) else {
            return Err(OwnershipAnalysisError::MissingLocalType(binding));
        };
        Ok(self
            .signatures
            .types()
            .ownership_root(local.ty())?
            .is_some())
    }

    fn binding_is_reference(
        &self,
        binding: LocalBindingId,
    ) -> Result<bool, OwnershipAnalysisError> {
        let FactState::Known(local) = self.body_types.local(binding) else {
            return Err(OwnershipAnalysisError::MissingLocalType(binding));
        };
        Ok(matches!(
            self.signatures.types().kind(local.ty())?,
            TypeKind::Reference { .. }
        ))
    }

    fn binding_contains_reference(
        &self,
        binding: LocalBindingId,
    ) -> Result<bool, OwnershipAnalysisError> {
        let FactState::Known(local) = self.body_types.local(binding) else {
            return Err(OwnershipAnalysisError::MissingLocalType(binding));
        };
        Ok(self.signatures.types().contains_reference(local.ty())?)
    }
}
