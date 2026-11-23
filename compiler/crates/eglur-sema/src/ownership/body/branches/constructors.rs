use super::*;
use crate::body_types::ConstructorSet;
use std::collections::BTreeSet;

#[cfg(test)]
mod tests;

pub(in super::super) enum MatchPaths {
    Independent(Alternatives),
    Ordered,
}

impl MatchPaths {
    pub(in super::super) fn begin(
        state: &mut FlowState,
        owner: ExpressionKey,
        whole_guards: bool,
    ) -> Self {
        if !whole_guards
            || !state.has_input()
            || state
                .search
                .as_ref()
                .is_some_and(|search| state.guard.is_some() || !search.current.position.can_nest())
        {
            return Self::Independent(Alternatives::begin(state));
        }

        let frame = SearchFrame::new(owner, state.split_guard_stamp(), state.before.take());
        let writes = std::mem::take(&mut state.bindings);
        if let Some(search) = &mut state.search {
            search.entries.parked.push(ParkedArm {
                frame: std::mem::replace(&mut search.current, frame),
                writes,
            });
        } else {
            state.search = Some(Box::new(ConstructorPaths {
                current: frame,
                entries: ArmEntries {
                    base: writes,
                    parked: Vec::new(),
                    overrides: BTreeSet::new(),
                },
            }));
        }
        Self::Ordered
    }

    pub(in super::super) fn enter(
        &self,
        state: &mut FlowState,
        constructors: Option<ConstructorSet>,
    ) {
        if let Self::Ordered = self {
            let routes = state.search.as_mut().expect("ordered search");
            routes.current.position =
                ArmPosition::Matching(constructors.expect("whole constructor entries"));
        }
    }

    pub(in super::super) fn select_body(&self, state: &mut FlowState) {
        if let Self::Ordered = self {
            let frame = &mut state.search.as_mut().expect("ordered search").current;
            frame.position = ArmPosition::Selected(frame.position.constructors());
        }
    }

    pub(in super::super) fn guard_false(
        &self,
        state: &mut FlowState,
        guard: GuardScope,
        mut outer: impl FnMut(LocalBindingId) -> Option<bool>,
    ) -> bool {
        match self {
            Self::Independent(branches) => !branches
                .changed_bindings(state, guard)
                .any(|id| outer(id) != Some(false)),
            Self::Ordered => {
                let routes = state.search.as_mut().expect("ordered search");
                let mut supported = true;
                state.bindings.retain(|&id, value| {
                    match outer(id) {
                        Some(true) => {}
                        Some(false) => return true,
                        None => {
                            supported = false;
                            return true;
                        }
                    }
                    let entry = routes.entry_value(id);
                    if guard.split && value.provenance.depends_on(guard.stamp) {
                        supported = false;
                    }
                    if BindingState::equivalent(Some(value), entry) {
                        routes.discard_write(id);
                        return false;
                    }
                    if guard.split && !value.same_flow(entry) {
                        supported = false;
                    }
                    routes.write(id, value);
                    false
                });
                supported
            }
        }
    }

    pub(in super::super) fn finish_arm(&mut self, state: &mut FlowState, continuing: bool) {
        match self {
            Self::Independent(branches) => branches.finish_arm(state, continuing),
            Self::Ordered => {
                let writes = std::mem::take(&mut state.bindings);
                let routes = state.search.as_mut().expect("ordered search");
                if continuing {
                    routes.sample(writes);
                } else {
                    for id in writes.into_keys() {
                        routes.discard_write(id);
                    }
                }
                routes.current.position = ArmPosition::Matching(ConstructorSet([false; 2]));
            }
        }
    }

    pub(in super::super) fn finish(self, state: &mut FlowState) -> bool {
        match self {
            Self::Independent(branches) => branches.finish(state),
            Self::Ordered => {
                let mut routes = state.search.take().expect("ordered search");
                debug_assert!(state.bindings.is_empty());
                let frame = if let Some(parent) = routes.entries.parked.pop() {
                    let depth = routes.entries.parked.len() + 1;
                    let finished = std::mem::replace(&mut routes.current, parent.frame);
                    for &id in finished.cells.keys() {
                        routes.entries.overrides.remove(&(id, depth));
                    }
                    state.bindings = parent.writes;
                    state.search = Some(routes);
                    finished
                } else {
                    state.bindings = routes.entries.base;
                    routes.current
                };
                state.before = frame.enclosing;
                for (id, mut cell) in frame.cells {
                    cell.flush(frame.exits, frame.enclosing_guard);
                    if let Some(out) = cell.output
                        && (state.search.is_none()
                            || !BindingState::equivalent(Some(&out), state.binding(id)))
                    {
                        state.update(id, |value| *value = out);
                    }
                }
                frame.exits != [0; 2]
            }
        }
    }
}

pub(super) struct ConstructorPaths {
    current: SearchFrame,
    entries: ArmEntries,
}

struct ArmEntries {
    base: Bindings,
    parked: Vec<ParkedArm>,
    // Exactly the frames with an arm write or constructor cell for this binding.
    overrides: BTreeSet<(LocalBindingId, usize)>,
}

struct ParkedArm {
    frame: SearchFrame,
    writes: Bindings,
}

impl ArmEntries {
    fn inherited(&self, id: LocalBindingId) -> Option<&BindingState> {
        if !self.parked.is_empty()
            && let Some(&(_, depth)) = self
                .overrides
                .range((id, 0)..(id, self.parked.len()))
                .next_back()
        {
            let arm = &self.parked[depth];
            #[cfg(test)]
            tests::observe_parked_read();
            return Some(
                arm.writes
                    .get(&id)
                    .or_else(|| arm.frame.lane_value(id))
                    .expect("indexed binding in a selected constructor arm"),
            );
        }
        self.base.get(&id)
    }
}

struct SearchFrame {
    owner: ExpressionKey,
    enclosing_guard: Option<usize>,
    enclosing: Option<BeforeImages>,
    cells: BTreeMap<LocalBindingId, SearchCell>,
    position: ArmPosition,
    exits: [usize; 2],
}

impl SearchFrame {
    fn new(
        owner: ExpressionKey,
        enclosing_guard: Option<usize>,
        enclosing: Option<BeforeImages>,
    ) -> Self {
        Self {
            owner,
            enclosing_guard,
            enclosing,
            cells: BTreeMap::new(),
            position: ArmPosition::Matching(ConstructorSet([false; 2])),
            exits: [0; 2],
        }
    }

    fn lane_value(&self, id: LocalBindingId) -> Option<&BindingState> {
        let cell = self.cells.get(&id)?;
        match self.position.constructors().0 {
            [true, false] => Some(&cell.values[0]),
            [false, true] => Some(&cell.values[1]),
            [true, true] => Some(&cell.joined),
            [false, false] => None,
        }
    }
}

#[derive(Clone, Copy)]
enum ArmPosition {
    Matching(ConstructorSet),
    Selected(ConstructorSet),
}

impl ArmPosition {
    fn constructors(self) -> ConstructorSet {
        match self {
            Self::Matching(set) | Self::Selected(set) => set,
        }
    }

    // A pair of referents belongs to this search. Only a selected singleton
    // has no remaining outer constructor distinction for a child to misread.
    fn can_nest(self) -> bool {
        matches!(
            self,
            Self::Selected(ConstructorSet([true, false] | [false, true]))
        )
    }
}

struct SearchCell {
    values: [BindingState; 2],
    joined: BindingState,
    sampled: [usize; 2],
    output: Option<BindingState>,
}

impl SearchCell {
    fn new(base: BindingState) -> Self {
        Self {
            values: [base.clone(), base.clone()],
            joined: base,
            sampled: [0; 2],
            output: None,
        }
    }

    fn sample_value(&mut self, value: BindingState, guard: Option<usize>) {
        #[cfg(test)]
        tests::observe_sample(&value);
        if let Some(out) = &mut self.output {
            out.join_choice(value, guard);
        } else {
            self.output = Some(value);
        }
    }

    // Repeated normal exits with the same value contribute only once to a join.
    fn flush(&mut self, exits: [usize; 2], guard: Option<usize>) {
        for (i, count) in exits.into_iter().enumerate() {
            if self.sampled[i] < count {
                self.sample_value(self.values[i].clone(), guard);
                self.sampled[i] = count;
            }
        }
    }
}

impl ConstructorPaths {
    pub(super) fn owner(&self) -> ExpressionKey {
        self.current.owner
    }

    pub(super) fn begin_write(&mut self, id: LocalBindingId) -> BindingState {
        self.entries
            .overrides
            .insert((id, self.entries.parked.len()));
        self.entry_value(id).cloned().unwrap_or_default()
    }

    pub(super) fn discard_write(&mut self, id: LocalBindingId) {
        if !self.current.cells.contains_key(&id) {
            self.entries
                .overrides
                .remove(&(id, self.entries.parked.len()));
        }
    }

    pub(super) fn entry_value(&self, id: LocalBindingId) -> Option<&BindingState> {
        self.current
            .lane_value(id)
            .or_else(|| self.entries.inherited(id))
    }

    fn cell(&mut self, id: LocalBindingId) -> &mut SearchCell {
        self.current.cells.entry(id).or_insert_with(|| {
            SearchCell::new(self.entries.inherited(id).cloned().unwrap_or_default())
        })
    }

    fn write(&mut self, id: LocalBindingId, value: &BindingState) {
        let (active, exits, guard) = (
            self.current.position.constructors().0,
            self.current.exits,
            self.current.enclosing_guard,
        );
        let cell = self.cell(id);
        cell.flush(exits, guard);
        let entry_availability = match active {
            [true, false] => cell.values[0].availability,
            [false, true] => cell.values[1].availability,
            _ => cell.joined.availability,
        };
        for (i, selected) in active.into_iter().enumerate() {
            if selected {
                if entry_availability != value.availability {
                    cell.values[i].availability = value.availability;
                }
                cell.values[i].provenance = value.provenance.for_constructor(i);
            }
        }
        cell.joined = BindingState {
            availability: cell.values[0]
                .availability
                .join(cell.values[1].availability),
            provenance: ValueProvenance::from_constructors(
                &cell.values[0].provenance,
                &cell.values[1].provenance,
            ),
        };
    }

    fn sample(&mut self, writes: Bindings) {
        let (active, exits, guard) = (
            self.current.position.constructors().0,
            self.current.exits,
            self.current.enclosing_guard,
        );
        for (id, mut value) in writes {
            let cell = self.cell(id);
            cell.flush(exits, guard);
            value.provenance.forget_constructors(guard);
            cell.sample_value(value, guard);
            for (i, selected) in active.into_iter().enumerate() {
                cell.sampled[i] += usize::from(selected);
            }
        }
        for (i, selected) in active.into_iter().enumerate() {
            self.current.exits[i] += usize::from(selected);
        }
    }

    pub(super) fn materialize(&self, writes: &Bindings) -> Bindings {
        #[cfg(test)]
        tests::observe_snapshot();
        let mut bindings = self.entries.base.clone();
        let mut previous = None;
        for &(id, _) in &self.entries.overrides {
            if previous == Some(id) {
                continue;
            }
            previous = Some(id);
            if let Some(value) = writes.get(&id).or_else(|| self.entry_value(id)) {
                if value.is_default() {
                    bindings.remove(&id);
                } else {
                    bindings.insert(id, value.clone());
                }
            }
        }
        bindings
    }
}
