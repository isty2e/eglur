use super::*;
use crate::body_types::ConstructorSet;

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
        if whole_guards && state.has_input() && state.search.is_none() {
            state.search = Some(Box::new(ConstructorPaths {
                owner,
                enclosing_guard: state.split_guard_stamp(),
                base: std::mem::take(&mut state.bindings),
                enclosing: state.before.take(),
                cells: BTreeMap::new(),
                active: ConstructorSet([false; 2]),
                exits: [0; 2],
            }));
            Self::Ordered
        } else {
            Self::Independent(Alternatives::begin(state))
        }
    }

    pub(in super::super) fn enter(
        &self,
        state: &mut FlowState,
        constructors: Option<ConstructorSet>,
    ) {
        if let Self::Ordered = self {
            let routes = state.search.as_mut().expect("ordered search");
            routes.active = constructors.expect("whole constructor entries");
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
                if continuing {
                    state
                        .search
                        .as_mut()
                        .expect("ordered search")
                        .sample(writes);
                }
            }
        }
    }

    pub(in super::super) fn finish(self, state: &mut FlowState) -> bool {
        match self {
            Self::Independent(branches) => branches.finish(state),
            Self::Ordered => {
                let routes = *state.search.take().expect("ordered search");
                debug_assert!(state.bindings.is_empty());
                state.bindings = routes.base;
                state.before = routes.enclosing;
                for (id, mut cell) in routes.cells {
                    cell.flush(routes.exits, routes.enclosing_guard);
                    if let Some(out) = cell.output {
                        state.update(id, |value| *value = out);
                    }
                }
                routes.exits != [0; 2]
            }
        }
    }
}

pub(super) struct ConstructorPaths {
    pub(super) owner: ExpressionKey,
    enclosing_guard: Option<usize>,
    base: Bindings,
    enclosing: Option<BeforeImages>,
    cells: BTreeMap<LocalBindingId, SearchCell>,
    active: ConstructorSet,
    exits: [usize; 2],
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
    pub(super) fn entry_value(&self, id: LocalBindingId) -> Option<&BindingState> {
        match (self.cells.get(&id), self.active.0) {
            (Some(cell), [true, false]) => Some(&cell.values[0]),
            (Some(cell), [false, true]) => Some(&cell.values[1]),
            (Some(cell), [true, true]) => Some(&cell.joined),
            _ => self.base.get(&id),
        }
    }

    fn cell(&mut self, id: LocalBindingId) -> &mut SearchCell {
        self.cells
            .entry(id)
            .or_insert_with(|| SearchCell::new(self.base.get(&id).cloned().unwrap_or_default()))
    }

    fn write(&mut self, id: LocalBindingId, value: &BindingState) {
        let (active, exits, guard) = (self.active.0, self.exits, self.enclosing_guard);
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
        let (active, exits, guard) = (self.active.0, self.exits, self.enclosing_guard);
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
            self.exits[i] += usize::from(selected);
        }
    }

    pub(super) fn materialize(&self) -> Bindings {
        #[cfg(test)]
        tests::observe_snapshot();
        let mut bindings = self.base.clone();
        for &id in self.cells.keys() {
            if let Some(value) = self.entry_value(id) {
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
