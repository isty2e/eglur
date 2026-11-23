use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Weak};

use crate::root::RootIdentity;
use crate::{
    Diagnostic, DiagnosticCode, DiagnosticRoot, DiagnosticSnapshot, ErrorLimit, Severity,
    SourceSpan,
};

#[derive(Debug)]
struct StoreIdentity;

/// Position in one diagnostic store's root-discovery sequence.
#[derive(Clone)]
pub struct DiagnosticCheckpoint {
    store: Arc<StoreIdentity>,
    next_root: usize,
}

struct SharedRootSnapshot {
    roots: Box<[usize]>,
}

#[derive(Clone)]
enum RootSnapshot {
    Single(usize),
    Shared(Arc<SharedRootSnapshot>),
}

impl RootSnapshot {
    fn roots(&self) -> &[usize] {
        match self {
            Self::Single(root) => std::slice::from_ref(root),
            Self::Shared(snapshot) => &snapshot.roots,
        }
    }

    fn same_snapshot(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Single(left), Self::Single(right)) => left == right,
            (Self::Shared(left), Self::Shared(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
}

/// Non-empty opaque provenance for facts unavailable after retained roots.
#[derive(Clone)]
pub struct Poison {
    store: Arc<StoreIdentity>,
    snapshot: RootSnapshot,
}

impl Poison {
    fn from_roots(store: Arc<StoreIdentity>, roots: Box<[usize]>) -> Self {
        #[cfg(test)]
        poison_tests::observe(poison_tests::Step::Snapshot(roots.len()));

        let snapshot = match roots.as_ref() {
            [root] => RootSnapshot::Single(*root),
            _ => RootSnapshot::Shared(Arc::new(SharedRootSnapshot { roots })),
        };

        Self { store, snapshot }
    }

    fn single(store: Arc<StoreIdentity>, root: usize) -> Self {
        #[cfg(test)]
        poison_tests::observe(poison_tests::Step::Snapshot(1));

        Self {
            store,
            snapshot: RootSnapshot::Single(root),
        }
    }

    fn roots(&self) -> &[usize] {
        self.snapshot.roots()
    }

    /// Whether both values trace to exactly the same failures in the same store.
    pub fn same_roots(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.store, &other.store)
            && (self.snapshot.same_snapshot(&other.snapshot) || self.roots() == other.roots())
    }

    fn contains_root(&self, root: &usize) -> bool {
        self.roots()
            .binary_search_by(|candidate| {
                #[cfg(test)]
                suppression_tests::observe_comparison();

                candidate.cmp(root)
            })
            .is_ok()
    }

    /// Merges two provenance sets from the same diagnostic store.
    pub fn merge(&self, other: &Self) -> Result<Self, DiagnosticStoreError> {
        if !Arc::ptr_eq(&self.store, &other.store) {
            return Err(DiagnosticStoreError::ForeignPoison);
        }

        if self.snapshot.same_snapshot(&other.snapshot) {
            return Ok(self.clone());
        }

        let mut roots = Vec::with_capacity(self.roots().len() + other.roots().len());
        let (mut left, mut right) = (0, 0);
        while left < self.roots().len() && right < other.roots().len() {
            match self.roots()[left].cmp(&other.roots()[right]) {
                std::cmp::Ordering::Less => {
                    roots.push(self.roots()[left]);
                    left += 1;
                }
                std::cmp::Ordering::Greater => {
                    roots.push(other.roots()[right]);
                    right += 1;
                }
                std::cmp::Ordering::Equal => {
                    roots.push(self.roots()[left]);
                    left += 1;
                    right += 1;
                }
            }
        }
        roots.extend_from_slice(&self.roots()[left..]);
        roots.extend_from_slice(&other.roots()[right..]);

        Ok(Self::from_roots(
            Arc::clone(&self.store),
            roots.into_boxed_slice(),
        ))
    }
}

impl fmt::Debug for Poison {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Poison")
            .field("root_count", &self.roots().len())
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
enum PendingRoots {
    #[default]
    Empty,
    Shared(Poison),
    Union {
        store: Arc<StoreIdentity>,
        roots: BTreeSet<usize>,
        seen: BTreeMap<usize, Weak<SharedRootSnapshot>>,
    },
}

/// Accumulates provenance, sharing repeated inputs without copying their roots.
/// Independently produced unions are not interned across accumulators.
#[derive(Default)]
pub struct PoisonAccumulator {
    pending: PendingRoots,
}

impl PoisonAccumulator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Rejects foreign provenance before changing the accumulated roots.
    pub fn include(&mut self, poison: &Poison) -> Result<(), DiagnosticStoreError> {
        #[cfg(test)]
        poison_tests::observe(poison_tests::Step::Input);

        let store = match &self.pending {
            PendingRoots::Empty => None,
            PendingRoots::Shared(first) => Some(&first.store),
            PendingRoots::Union { store, .. } => Some(store),
        };
        if let Some(store) = store {
            #[cfg(test)]
            poison_tests::observe(poison_tests::Step::PoolCheck);

            if !Arc::ptr_eq(store, &poison.store) {
                #[cfg(test)]
                poison_tests::observe(poison_tests::Step::Foreign);

                return Err(DiagnosticStoreError::ForeignPoison);
            }
        }

        match &mut self.pending {
            PendingRoots::Empty => self.pending = PendingRoots::Shared(poison.clone()),
            PendingRoots::Shared(first) if first.snapshot.same_snapshot(&poison.snapshot) => {}
            PendingRoots::Shared(first) => {
                let mut roots = BTreeSet::new();
                let mut seen = BTreeMap::new();
                Self::include_snapshot(&mut roots, &mut seen, first);
                Self::include_snapshot(&mut roots, &mut seen, poison);
                self.pending = PendingRoots::Union {
                    store: Arc::clone(&first.store),
                    roots,
                    seen,
                };
            }
            PendingRoots::Union { roots, seen, .. } => Self::include_snapshot(roots, seen, poison),
        }

        Ok(())
    }

    fn include_snapshot(
        roots: &mut BTreeSet<usize>,
        seen: &mut BTreeMap<usize, Weak<SharedRootSnapshot>>,
        poison: &Poison,
    ) {
        match &poison.snapshot {
            RootSnapshot::Single(root) => {
                #[cfg(test)]
                poison_tests::observe(poison_tests::Step::Root);

                roots.insert(*root);
            }
            RootSnapshot::Shared(snapshot) => {
                let id = Arc::as_ptr(snapshot).addr();
                if let std::collections::btree_map::Entry::Vacant(entry) = seen.entry(id) {
                    // The weak header prevents address reuse without retaining the boxed roots.
                    entry.insert(Arc::downgrade(snapshot));
                    for &root in &snapshot.roots {
                        #[cfg(test)]
                        poison_tests::observe(poison_tests::Step::Root);

                        roots.insert(root);
                    }
                }
            }
        }
    }

    /// Freezes one sorted union, or returns `None` when no input was included.
    #[must_use]
    pub fn finish(self) -> Option<Poison> {
        match self.pending {
            PendingRoots::Empty => None,
            PendingRoots::Shared(poison) => {
                #[cfg(test)]
                poison_tests::observe(poison_tests::Step::Finish(0));

                Some(poison)
            }
            PendingRoots::Union { store, roots, .. } => {
                #[cfg(test)]
                poison_tests::observe(poison_tests::Step::Finish(roots.len()));

                Some(Poison::from_roots(
                    store,
                    roots.into_iter().collect::<Vec<_>>().into_boxed_slice(),
                ))
            }
        }
    }
}

/// Phase-owned classification of one failed semantic check.
#[derive(Debug)]
pub enum DiagnosticFailure {
    /// An independently established failed obligation.
    Root(DiagnosticRoot),
    /// A check that cannot decide without already-poisoned prerequisites.
    Derivative(Poison),
}

/// Internal contract failure while retaining diagnostics or poison.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticStoreError {
    /// The same root identity was rediscovered with a different record.
    ConflictingRoot(DiagnosticCode),
    /// Poison from another store was used as local provenance.
    ForeignPoison,
    /// An error attempted to bypass independent-root classification.
    ErrorObservation,
    /// A checkpoint from another diagnostic store was used.
    ForeignCheckpoint,
    /// All claimed earlier roots have already been superseded.
    NoPrecedingRoot,
}

impl fmt::Display for DiagnosticStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConflictingRoot(code) => write!(
                formatter,
                "diagnostic root {code} was rediscovered with conflicting record data",
            ),
            Self::ForeignPoison => {
                formatter.write_str("poison provenance belongs to another diagnostic store")
            }
            Self::ErrorObservation => {
                formatter.write_str("error diagnostics must be recorded as independent roots")
            }
            Self::ForeignCheckpoint => {
                formatter.write_str("diagnostic checkpoint belongs to another store")
            }
            Self::NoPrecedingRoot => {
                formatter.write_str("no retained preceding diagnostic root exists")
            }
        }
    }
}

impl std::error::Error for DiagnosticStoreError {}

#[derive(Debug)]
struct StoredRoot {
    id: usize,
    diagnostic: Diagnostic,
}

/// Single mutable authority for diagnostic roots and poisoned provenance.
pub struct DiagnosticStore {
    identity: Arc<StoreIdentity>,
    roots: HashMap<RootIdentity, StoredRoot>,
    root_order: Vec<RootIdentity>,
    superseded: HashSet<usize>,
    observations: Vec<Diagnostic>,
    next_root: usize,
}

impl DiagnosticStore {
    /// Creates an empty independent diagnostic lifecycle.
    #[must_use]
    pub fn new() -> Self {
        Self {
            identity: Arc::new(StoreIdentity),
            roots: HashMap::new(),
            root_order: Vec::new(),
            superseded: HashSet::new(),
            observations: Vec::new(),
            next_root: 0,
        }
    }

    /// Records a root or propagates a derivative without emitting another one.
    pub fn record_failure(
        &mut self,
        failure: DiagnosticFailure,
    ) -> Result<Poison, DiagnosticStoreError> {
        match failure {
            DiagnosticFailure::Root(root) => self.record_root(root),
            DiagnosticFailure::Derivative(poison) => {
                self.validate_poison(&poison)?;
                Ok(poison)
            }
        }
    }

    /// Retains a warning or note that is not part of root/poison semantics.
    pub fn record_observation(
        &mut self,
        diagnostic: Diagnostic,
    ) -> Result<(), DiagnosticStoreError> {
        if diagnostic.severity() == Severity::Error {
            return Err(DiagnosticStoreError::ErrorObservation);
        }
        self.observations.push(diagnostic);
        Ok(())
    }

    /// Reports whether at least one independently established error root exists.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.roots
            .values()
            .any(|root| !self.superseded.contains(&root.id))
    }

    /// Marks the beginning of a local validation stage without copying diagnostics.
    #[must_use]
    pub fn checkpoint(&self) -> DiagnosticCheckpoint {
        DiagnosticCheckpoint {
            store: Arc::clone(&self.identity),
            next_root: self.next_root,
        }
    }

    /// Returns only newly discovered roots physically inside a source construct.
    pub fn roots_since_within(
        &self,
        checkpoint: &DiagnosticCheckpoint,
        construct: &SourceSpan,
    ) -> Result<Option<Poison>, DiagnosticStoreError> {
        if !Arc::ptr_eq(&self.identity, &checkpoint.store) {
            return Err(DiagnosticStoreError::ForeignCheckpoint);
        }
        let roots = self.root_order[checkpoint.next_root..]
            .iter()
            .enumerate()
            .filter_map(|(offset, identity)| {
                let RootIdentity::Source { source, bytes, .. } = identity else {
                    return None;
                };
                let container = construct.file_span();
                (source == construct.source()
                    && self.roots[identity]
                        .diagnostic
                        .primary_span()
                        .as_source()
                        .is_some_and(|span| span.file_span().file() == container.file())
                    && bytes.start() >= container.range().start()
                    && bytes.end() <= container.range().end())
                .then_some(checkpoint.next_root + offset)
            })
            .collect::<Vec<_>>();
        Ok((!roots.is_empty())
            .then(|| Poison::from_roots(Arc::clone(&self.identity), roots.into_boxed_slice())))
    }

    /// Suppresses the specified later roots when an earlier root remains retained.
    pub fn select_preceding_roots(
        &mut self,
        preceding: &Poison,
        later: &Poison,
    ) -> Result<(), DiagnosticStoreError> {
        self.validate_poison(preceding)?;
        self.validate_poison(later)?;
        if preceding
            .roots()
            .iter()
            .all(|root| self.superseded.contains(root))
        {
            return Err(DiagnosticStoreError::NoPrecedingRoot);
        }
        for root in later
            .roots()
            .iter()
            .filter(|root| !preceding.contains_root(root))
        {
            self.superseded.insert(*root);
        }
        Ok(())
    }

    /// Consumes discovery state and freezes one canonically ordered snapshot.
    #[must_use]
    pub fn finalize(self, limit: Option<ErrorLimit>) -> DiagnosticSnapshot {
        let errors = self
            .roots
            .into_values()
            .filter(|root| !self.superseded.contains(&root.id))
            .map(|root| root.diagnostic)
            .collect();
        DiagnosticSnapshot::finalize(errors, self.observations, limit)
    }

    fn record_root(&mut self, root: DiagnosticRoot) -> Result<Poison, DiagnosticStoreError> {
        let (identity, diagnostic) = root.into_parts();
        let next_root = self.next_root;
        let root_id = match self.roots.entry(identity.clone()) {
            Entry::Occupied(entry) => {
                let established = entry.get();
                if established.diagnostic != diagnostic {
                    return Err(DiagnosticStoreError::ConflictingRoot(
                        diagnostic.code().clone(),
                    ));
                }
                established.id
            }
            Entry::Vacant(entry) => {
                self.next_root = self
                    .next_root
                    .checked_add(1)
                    .expect("diagnostic root count exceeds platform capacity");
                entry.insert(StoredRoot {
                    id: next_root,
                    diagnostic,
                });
                self.root_order.push(identity);
                next_root
            }
        };
        Ok(Poison::single(Arc::clone(&self.identity), root_id))
    }

    fn validate_poison(&self, poison: &Poison) -> Result<(), DiagnosticStoreError> {
        if Arc::ptr_eq(&self.identity, &poison.store) {
            Ok(())
        } else {
            Err(DiagnosticStoreError::ForeignPoison)
        }
    }
}

impl Default for DiagnosticStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod poison_tests;

#[cfg(test)]
mod suppression_tests;
