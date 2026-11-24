use eglur_hir::OriginId;

use crate::{ControlRegionKey, DefId, FactState, FactTable, LocalBindingId};

/// Function-local availability in the closed v0.1 ownership lattice.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum OwnershipAvailability {
    Owned,
    Consumed,
    MaybeConsumed,
}

impl OwnershipAvailability {
    #[must_use]
    pub(crate) const fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Owned, Self::Owned) => Self::Owned,
            (Self::Consumed, Self::Consumed) => Self::Consumed,
            _ => Self::MaybeConsumed,
        }
    }
}

/// Whether one callable's ownership contract was fully checked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipBodyStatus {
    Checked,
    Unperformed,
}

/// Stable named ownership obligation discovered in one callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnershipRootFact {
    binding: LocalBindingId,
    region: ControlRegionKey,
}

impl OwnershipRootFact {
    pub(crate) const fn new(binding: LocalBindingId, region: ControlRegionKey) -> Self {
        Self { binding, region }
    }

    #[must_use]
    pub const fn binding(&self) -> LocalBindingId {
        self.binding
    }

    #[must_use]
    pub const fn region(&self) -> &ControlRegionKey {
        &self.region
    }
}

/// Bounded ownership-analysis result for one source callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnershipBodySummary {
    status: OwnershipBodyStatus,
    roots: Box<[OwnershipRootFact]>,
    operation_count: usize,
}

impl OwnershipBodySummary {
    pub(crate) fn new(
        status: OwnershipBodyStatus,
        roots: impl IntoIterator<Item = OwnershipRootFact>,
        operation_count: usize,
    ) -> Self {
        Self {
            status,
            roots: roots.into_iter().collect::<Vec<_>>().into_boxed_slice(),
            operation_count,
        }
    }

    #[must_use]
    pub const fn status(&self) -> OwnershipBodyStatus {
        self.status
    }

    #[must_use]
    pub fn roots(&self) -> &[OwnershipRootFact] {
        &self.roots
    }

    #[must_use]
    pub const fn operation_count(&self) -> usize {
        self.operation_count
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum OwnershipFailureKind {
    TransferRequiresMove,
    DoubleMove,
    UseAfterDischarge,
    PathUnbalanced,
    GuardConsume,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OwnershipCause {
    Binding {
        kind: OwnershipFailureKind,
        binding: LocalBindingId,
        first_consume: Option<OriginId>,
    },
    PendingMatchExit {
        scrutinee: OriginId,
    },
    UnboundMatchPayload {
        scrutinee: OriginId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnershipFailure {
    owner: DefId,
    cause: OwnershipCause,
    origin: OriginId,
    closing_delimiter: bool,
}

impl OwnershipFailure {
    pub(crate) const fn new(
        owner: DefId,
        kind: OwnershipFailureKind,
        origin: OriginId,
        binding: LocalBindingId,
        first_consume: Option<OriginId>,
    ) -> Self {
        Self::at(
            owner,
            origin,
            OwnershipCause::Binding {
                kind,
                binding,
                first_consume,
            },
        )
    }

    pub(super) const fn at(owner: DefId, origin: OriginId, cause: OwnershipCause) -> Self {
        Self {
            owner,
            cause,
            origin,
            closing_delimiter: false,
        }
    }

    pub(crate) const fn at_closing_delimiter(mut self) -> Self {
        self.closing_delimiter = true;
        self
    }

    pub(crate) const fn owner(&self) -> DefId {
        self.owner
    }

    pub(super) const fn cause(&self) -> OwnershipCause {
        self.cause
    }

    pub(crate) const fn origin(&self) -> OriginId {
        self.origin
    }

    pub(crate) const fn closing_delimiter(&self) -> bool {
        self.closing_delimiter
    }
}

/// Callable-local ownership products retained after diagnostics are emitted.
pub struct OwnershipFacts {
    pub(crate) bodies: FactTable<DefId, OwnershipBodySummary>,
    pub(crate) complete: bool,
}

impl OwnershipFacts {
    pub(crate) fn new() -> Self {
        Self {
            bodies: FactTable::new(),
            complete: true,
        }
    }

    #[must_use]
    pub fn body(&self, owner: DefId) -> FactState<'_, OwnershipBodySummary> {
        self.bodies.state(&owner)
    }

    pub fn bodies(
        &self,
    ) -> impl ExactSizeIterator<Item = (DefId, FactState<'_, OwnershipBodySummary>)> {
        self.bodies
            .iter()
            .map(|(owner, _)| (*owner, self.bodies.state(owner)))
    }

    #[must_use]
    pub fn fact_count(&self) -> usize {
        self.bodies.iter().len()
    }

    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }

    pub(crate) fn has_poisoned_facts(&self) -> bool {
        self.bodies.has_poisoned()
    }
}
