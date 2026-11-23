use std::collections::{BTreeMap, btree_map::Entry};

use super::{AvailabilityFact, LocalBindingId, ValueProvenance};
use crate::ExpressionKey;

mod constructors;
use constructors::ConstructorPaths;
pub(super) use constructors::MatchPaths;

#[derive(Clone, Default, Eq, PartialEq)]
struct BindingState {
    availability: AvailabilityFact,
    provenance: ValueProvenance,
}

impl BindingState {
    fn join_choice(&mut self, other: Self, guard: Option<usize>) {
        self.availability = self.availability.join(other.availability);
        self.provenance.join_choice(other.provenance, guard);
    }

    fn same_flow(&self, other: Option<&Self>) -> bool {
        other.map_or_else(
            || self.is_default(),
            |other| {
                self.availability == other.availability
                    && self.provenance.same_sources(&other.provenance)
            },
        )
    }

    fn is_default(&self) -> bool {
        self.availability == AvailabilityFact::owned() && self.provenance.is_empty()
    }

    fn equivalent(a: Option<&Self>, b: Option<&Self>) -> bool {
        match (a, b) {
            (Some(a), Some(b)) => a == b,
            (Some(value), None) | (None, Some(value)) => value.is_default(),
            (None, None) => true,
        }
    }
}

type Bindings = BTreeMap<LocalBindingId, BindingState>;
type BeforeImages = BTreeMap<LocalBindingId, Option<BindingState>>;

#[derive(Clone, Copy)]
pub(super) struct GuardScope {
    stamp: usize,
    outermost: usize,
    split: bool,
}

impl GuardScope {
    pub(super) fn new(stamp: usize) -> Self {
        debug_assert_ne!(stamp, 0);
        Self {
            stamp,
            outermost: stamp,
            split: false,
        }
    }
}

#[derive(Default)]
pub(super) struct FlowState {
    bindings: Bindings,
    before: Option<BeforeImages>,
    search: Option<Box<ConstructorPaths>>,
    guard: Option<GuardScope>,
    written_only: bool,
}

impl FlowState {
    pub(super) fn begin_guard(&mut self, stamp: usize) -> Option<GuardScope> {
        let mut guard = GuardScope::new(stamp);
        if let Some(parent) = self.guard {
            guard.outermost = parent.outermost;
        }
        self.guard.replace(guard)
    }

    pub(super) fn end_guard(&mut self, parent: Option<GuardScope>) -> GuardScope {
        std::mem::replace(&mut self.guard, parent).expect("active guard")
    }

    pub(super) fn note_guard_split(&mut self) {
        if let Some(guard) = &mut self.guard {
            guard.split = true;
        }
    }

    pub(super) fn split_guard_stamp(&self) -> Option<usize> {
        self.guard
            .filter(|guard| guard.split)
            .map(|guard| guard.stamp)
    }

    pub(super) fn written_only() -> Self {
        Self {
            written_only: true,
            ..Self::default()
        }
    }

    pub(super) fn has_input(&self) -> bool {
        !self.written_only
    }

    fn binding(&self, id: LocalBindingId) -> Option<&BindingState> {
        self.bindings
            .get(&id)
            .or_else(|| self.search.as_ref()?.entry_value(id))
    }

    pub(super) fn availability(&self, id: LocalBindingId) -> AvailabilityFact {
        self.binding(id)
            .map_or_else(AvailabilityFact::owned, |v| v.availability)
    }

    pub(super) fn alias(&self, id: LocalBindingId) -> Option<&ValueProvenance> {
        self.binding(id).map(|value| &value.provenance)
    }

    pub(super) fn set_availability(&mut self, id: LocalBindingId, fact: AvailabilityFact) {
        self.update(id, |value| value.availability = fact);
    }

    pub(super) fn set_alias(&mut self, id: LocalBindingId, provenance: ValueProvenance) {
        self.update(id, |value| value.provenance = provenance);
    }

    fn update(&mut self, id: LocalBindingId, change: impl FnOnce(&mut BindingState)) {
        if self.written_only {
            return;
        }
        self.remember(id);
        let value = self.bindings.entry(id).or_insert_with(|| {
            self.search
                .as_mut()
                .map(|search| search.begin_write(id))
                .unwrap_or_default()
        });
        change(value);
        if self.search.is_none() && value.is_default() {
            self.bindings.remove(&id);
        }
    }

    fn remember(&mut self, id: LocalBindingId) {
        if let Some(before) = &mut self.before {
            before
                .entry(id)
                .or_insert_with(|| self.bindings.get(&id).cloned());
        }
    }

    pub(super) fn end_binding(&mut self, id: LocalBindingId) {
        if self.binding(id).is_some() {
            self.update(id, |value| *value = BindingState::default());
        }
    }

    fn exit_snapshot(&self) -> Self {
        let bindings = self.search.as_ref().map_or_else(
            || self.bindings.clone(),
            |search| search.materialize(&self.bindings),
        );
        Self {
            bindings,
            ..Self::default()
        }
    }
}

#[derive(Default)]
struct PathJoin {
    paths: usize,
    contributions: BTreeMap<LocalBindingId, (usize, BindingState)>,
    guard: Option<usize>,
}

impl PathJoin {
    fn begin(state: &FlowState) -> Self {
        Self {
            guard: state.split_guard_stamp(),
            ..Self::default()
        }
    }

    fn contribute(&mut self, id: LocalBindingId, value: BindingState) {
        match self.contributions.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert((1, value));
            }
            Entry::Occupied(mut entry) => {
                let (count, joined) = entry.get_mut();
                *count += 1;
                joined.join_choice(value, self.guard);
            }
        }
    }

    fn apply(self, entry: &mut FlowState) {
        for (id, (count, mut joined)) in self.contributions {
            if count < self.paths {
                joined.join_choice(entry.binding(id).cloned().unwrap_or_default(), self.guard);
            }
            entry.update(id, |value| *value = joined);
        }
    }
}

/// All arms share the state after the subject. Only bindings written by the
/// current arm need a before-image; stopped arms rewind without contributing.
pub(super) struct Alternatives {
    enclosing: Option<BeforeImages>,
    join: PathJoin,
}

impl Alternatives {
    pub(super) fn begin(state: &mut FlowState) -> Self {
        Self {
            enclosing: state.before.replace(BeforeImages::new()),
            join: PathJoin::begin(state),
        }
    }

    pub(super) fn unchanged_path(&mut self) {
        self.join.paths += 1;
    }

    pub(super) fn changed_bindings<'a>(
        &self,
        state: &'a FlowState,
        guard: GuardScope,
    ) -> impl Iterator<Item = LocalBindingId> + 'a {
        state
            .before
            .as_ref()
            .expect("active branch")
            .iter()
            .filter_map(move |(&id, entry)| {
                let original = entry
                    .as_ref()
                    .or_else(|| state.search.as_ref()?.entry_value(id));
                let current = state.binding(id);
                let changed = current.map_or_else(
                    || original.is_some_and(|old| !old.is_default()),
                    |value| {
                        (guard.split && value.provenance.depends_on(guard.stamp))
                            || !value.same_flow(original)
                    },
                );
                let carries_enclosing_choice = state.guard.is_some_and(|parent| {
                    !BindingState::equivalent(original, current)
                        && [original, current]
                            .into_iter()
                            .flatten()
                            .any(|v| v.provenance.depends_on(parent.outermost))
                });
                (changed || carries_enclosing_choice).then_some(id)
            })
    }

    pub(super) fn finish_arm(&mut self, state: &mut FlowState, continuing: bool) {
        let before = std::mem::take(state.before.as_mut().expect("active branch"));
        if continuing {
            self.join.paths += 1;
        }
        for (id, original) in before {
            let outgoing = match original {
                Some(value) => state.bindings.insert(id, value),
                None => {
                    if let Some(search) = &mut state.search {
                        search.discard_write(id);
                    }
                    state.bindings.remove(&id)
                }
            };
            if continuing {
                let fallback = state.search.as_ref().and_then(|s| s.entry_value(id));
                if !BindingState::equivalent(outgoing.as_ref().or(fallback), state.binding(id)) {
                    self.join.contribute(
                        id,
                        outgoing.or_else(|| fallback.cloned()).unwrap_or_default(),
                    );
                }
            }
        }
    }

    pub(super) fn finish(self, state: &mut FlowState) -> bool {
        debug_assert!(state.before.as_ref().is_some_and(BTreeMap::is_empty));
        state.before = self.enclosing;
        let continuing = self.join.paths != 0;
        self.join.apply(state);
        continuing
    }
}

/// A recoverable edge has already exited its target region. Fold it as it
/// arrives so an enclosing result/checked block need not retain every arm.
pub(super) struct RecoverableExits {
    owner: Option<ExpressionKey>,
    join: PathJoin,
}

impl RecoverableExits {
    pub(super) fn begin(state: &FlowState) -> Self {
        Self {
            owner: state.search.as_ref().map(|s| s.owner()),
            join: PathJoin::begin(state),
        }
    }

    pub(super) fn include(
        &mut self,
        state: &FlowState,
        leaving: impl IntoIterator<Item = LocalBindingId>,
    ) {
        let mut snapshot = state.exit_snapshot();
        for id in leaving {
            snapshot.bindings.remove(&id);
        }
        let leaves_search = state.search.as_ref().map(|s| s.owner()) != self.owner;
        self.join.paths += 1;
        for (id, mut value) in snapshot.bindings {
            if leaves_search {
                value.provenance.forget_constructors(self.join.guard);
            }
            self.join.contribute(id, value);
        }
    }

    pub(super) fn finish(mut self, state: &mut FlowState, normal: bool) -> bool {
        if self.join.paths == 0 {
            return normal;
        }
        if normal {
            self.include(state, []);
        }
        let mut merged = FlowState::default();
        self.join.apply(&mut merged);
        if state.search.is_some() {
            // Caught exits already paid for full snapshots. Restore the joined facts
            // through the active overlay so later arm searches retain their own state.
            let current = state.exit_snapshot();
            for &id in current
                .bindings
                .keys()
                .filter(|id| !merged.bindings.contains_key(id))
            {
                state.end_binding(id);
            }
            for (id, joined) in merged.bindings {
                if current.bindings.get(&id) != Some(&joined) {
                    state.update(id, |value| *value = joined);
                }
            }
            return true;
        }
        if let Some(before) = &mut state.before {
            for &id in state.bindings.keys().chain(merged.bindings.keys()) {
                if state.bindings.get(&id) != merged.bindings.get(&id) {
                    before
                        .entry(id)
                        .or_insert_with(|| state.bindings.get(&id).cloned());
                }
            }
        }
        state.bindings = merged.bindings;
        true
    }
}
