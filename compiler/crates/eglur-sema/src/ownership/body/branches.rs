use std::collections::{BTreeMap, btree_map::Entry};

use super::{AvailabilityFact, LocalBindingId, ValueProvenance};

#[derive(Clone, Default, Eq, PartialEq)]
struct BindingState {
    availability: AvailabilityFact,
    provenance: ValueProvenance,
}

impl BindingState {
    fn join(&mut self, other: Self) {
        self.availability = self.availability.join(other.availability);
        self.provenance.extend(other.provenance);
    }

    fn is_default(&self) -> bool {
        self.availability == AvailabilityFact::owned() && self.provenance.is_empty()
    }
}

type Bindings = BTreeMap<LocalBindingId, BindingState>;
type BeforeImages = BTreeMap<LocalBindingId, Option<BindingState>>;

#[derive(Default)]
pub(super) struct FlowState {
    bindings: Bindings,
    before: Option<BeforeImages>,
}

impl FlowState {
    pub(super) fn availability(&self, id: LocalBindingId) -> AvailabilityFact {
        self.bindings
            .get(&id)
            .map_or_else(AvailabilityFact::owned, |v| v.availability)
    }

    pub(super) fn alias(&self, id: LocalBindingId) -> Option<&ValueProvenance> {
        self.bindings.get(&id).map(|value| &value.provenance)
    }

    pub(super) fn set_availability(&mut self, id: LocalBindingId, fact: AvailabilityFact) {
        self.update(id, |value| value.availability = fact);
    }

    pub(super) fn set_alias(&mut self, id: LocalBindingId, provenance: ValueProvenance) {
        self.update(id, |value| value.provenance = provenance);
    }

    fn update(&mut self, id: LocalBindingId, change: impl FnOnce(&mut BindingState)) {
        self.remember(id);
        let value = self.bindings.entry(id).or_default();
        change(value);
        if value.is_default() {
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
        if self.bindings.contains_key(&id) {
            self.remember(id);
            self.bindings.remove(&id);
        }
    }

    pub(super) fn exit_snapshot(&self) -> Self {
        Self {
            bindings: self.bindings.clone(),
            before: None,
        }
    }
}

#[derive(Default)]
struct PathJoin {
    paths: usize,
    contributions: BTreeMap<LocalBindingId, (usize, BindingState)>,
}

impl PathJoin {
    fn contribute(&mut self, id: LocalBindingId, value: BindingState) {
        match self.contributions.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert((1, value));
            }
            Entry::Occupied(mut entry) => {
                let (count, joined) = entry.get_mut();
                *count += 1;
                joined.join(value);
            }
        }
    }

    fn apply(self, entry: &mut FlowState) {
        for (id, (count, mut joined)) in self.contributions {
            if count < self.paths {
                joined.join(entry.bindings.get(&id).cloned().unwrap_or_default());
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
            join: PathJoin::default(),
        }
    }

    pub(super) fn unchanged_path(&mut self) {
        self.join.paths += 1;
    }

    pub(super) fn changed_bindings<'a>(
        &self,
        state: &'a FlowState,
    ) -> impl Iterator<Item = LocalBindingId> + 'a {
        state
            .before
            .as_ref()
            .expect("active branch")
            .iter()
            .filter_map(|(&id, entry)| (entry.as_ref() != state.bindings.get(&id)).then_some(id))
    }

    pub(super) fn finish_arm(&mut self, state: &mut FlowState, continuing: bool) {
        let before = std::mem::take(state.before.as_mut().expect("active branch"));
        if continuing {
            self.join.paths += 1;
        }
        for (id, original) in before {
            let outgoing = match original {
                Some(value) => state.bindings.insert(id, value),
                None => state.bindings.remove(&id),
            };
            if continuing && outgoing.as_ref() != state.bindings.get(&id) {
                self.join.contribute(id, outgoing.unwrap_or_default());
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
#[derive(Default)]
pub(super) struct RecoverableExits(PathJoin);

impl RecoverableExits {
    pub(super) fn include(&mut self, snapshot: FlowState) {
        self.0.paths += 1;
        for (id, value) in snapshot.bindings {
            self.0.contribute(id, value);
        }
    }

    pub(super) fn finish(mut self, state: &mut FlowState, normal: bool) -> bool {
        if self.0.paths == 0 {
            return normal;
        }
        if normal {
            self.include(state.exit_snapshot());
        }
        let mut merged = FlowState::default();
        self.0.apply(&mut merged);
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
