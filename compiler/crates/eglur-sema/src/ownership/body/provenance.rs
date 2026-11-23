use super::LocalBindingId;
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::ops::Bound;

/// Reads use the joined sources. Copies inside a shared guard also retain the
/// sources on each constructor so that a false exit can resume either search.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ValueProvenance {
    joined: ReferencePaths,
    separate: Option<Box<[ReferencePaths; 2]>>,
}

impl ValueProvenance {
    fn uniform(joined: ReferencePaths) -> Self {
        Self {
            joined,
            separate: None,
        }
    }

    pub(super) fn direct(roots: BTreeSet<LocalBindingId>) -> Self {
        Self::uniform(ReferencePaths::direct(roots))
    }

    pub(super) fn from_constructors(first: &Self, second: &Self) -> Self {
        debug_assert!(first.separate.is_none() && second.separate.is_none());
        if first == second {
            return first.clone();
        }
        let mut joined = first.joined.clone();
        joined.merge(second.joined.clone(), None);
        Self {
            joined,
            separate: Some(Box::new([first.joined.clone(), second.joined.clone()])),
        }
    }

    fn lane(&self, i: usize) -> &ReferencePaths {
        self.separate.as_ref().map_or(&self.joined, |pair| &pair[i])
    }

    pub(super) fn for_constructor(&self, i: usize) -> Self {
        Self::uniform(self.lane(i).clone())
    }

    pub(super) fn forget_constructors(&mut self, guard: Option<usize>) {
        if let Some(parts) = self.separate.take() {
            let [mut a, b] = *parts;
            a.merge(b, guard);
            self.joined = a;
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.joined.is_empty()
    }

    pub(super) fn roots(&self) -> BTreeSet<LocalBindingId> {
        self.joined.roots()
    }

    pub(super) fn same_sources(&self, other: &Self) -> bool {
        if self.separate.is_none() && other.separate.is_none() {
            self.joined.same_sources(&other.joined)
        } else {
            (0..2).all(|i| self.lane(i).same_sources(other.lane(i)))
        }
    }

    pub(super) fn depends_on(&self, guard: usize) -> bool {
        self.separate.as_ref().map_or_else(
            || self.joined.depends_on(guard),
            |parts| parts.iter().any(|lane| lane.depends_on(guard)),
        )
    }

    pub(super) fn join_choice(&mut self, other: Self, guard: Option<usize>) {
        if let Some(parts) = other.separate {
            let current = self
                .separate
                .get_or_insert_with(|| Box::new([self.joined.clone(), self.joined.clone()]));
            for (lane, addition) in current.iter_mut().zip(*parts) {
                lane.merge(addition, guard);
            }
        } else if let Some(current) = &mut self.separate {
            for lane in current.iter_mut() {
                lane.merge(other.joined.clone(), guard);
            }
        }
        self.joined.merge(other.joined, guard);
    }

    pub(super) fn extend(&mut self, other: Self) {
        self.join_choice(other, None);
    }

    pub(super) fn prefixed(self, index: usize) -> Self {
        Self {
            joined: self.joined.prefixed(index),
            separate: self
                .separate
                .map(|parts| Box::new(parts.map(|lane| lane.prefixed(index)))),
        }
    }

    fn map_paths(&self, f: impl Fn(&ReferencePaths) -> ReferencePaths) -> Self {
        Self {
            joined: f(&self.joined),
            separate: self
                .separate
                .as_ref()
                .map(|parts| Box::new(parts.each_ref().map(f))),
        }
    }

    pub(super) fn projected(&self, index: usize) -> Self {
        self.map_paths(|paths| paths.projected(index))
    }

    pub(super) fn borrowed(&self) -> Self {
        self.map_paths(ReferencePaths::borrowed)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ReferencePaths {
    roots_by_tuple_path: BTreeMap<Vec<usize>, Referents>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Referents {
    roots: BTreeSet<LocalBindingId>,
    // Newer choices were made by this guard or by a nested guard.
    last_choice: usize,
}

impl ReferencePaths {
    fn is_empty(&self) -> bool {
        self.roots_by_tuple_path.is_empty()
    }

    fn direct(roots: BTreeSet<LocalBindingId>) -> Self {
        let mut provenance = Self::default();
        if !roots.is_empty() {
            provenance.roots_by_tuple_path.insert(
                Vec::new(),
                Referents {
                    roots,
                    last_choice: 0,
                },
            );
        }
        provenance
    }

    fn depends_on(&self, guard: usize) -> bool {
        self.roots_by_tuple_path
            .values()
            .any(|sources| sources.last_choice >= guard)
    }

    fn same_sources(&self, other: &Self) -> bool {
        self.roots_by_tuple_path.len() == other.roots_by_tuple_path.len()
            && self
                .roots_by_tuple_path
                .iter()
                .zip(&other.roots_by_tuple_path)
                .all(|((a, left), (b, right))| a == b && left.roots == right.roots)
    }

    fn merge(&mut self, other: Self, guard: Option<usize>) {
        if let Some(guard) = guard {
            for (path, value) in &mut self.roots_by_tuple_path {
                if !other.roots_by_tuple_path.contains_key(path) {
                    value.last_choice = value.last_choice.max(guard);
                }
            }
        }
        for (path, mut addition) in other.roots_by_tuple_path {
            match self.roots_by_tuple_path.entry(path) {
                Entry::Vacant(entry) => {
                    addition.last_choice = addition.last_choice.max(guard.unwrap_or(0));
                    entry.insert(addition);
                }
                Entry::Occupied(entry) => {
                    let current = entry.into_mut();
                    current.last_choice = current.last_choice.max(addition.last_choice);
                    if let Some(guard) = guard
                        && current.roots != addition.roots
                    {
                        current.last_choice = current.last_choice.max(guard);
                    }
                    current.roots.extend(addition.roots);
                }
            }
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
                .insert(path[1..].to_vec(), roots.clone());
        }
        projected
    }

    fn roots(&self) -> BTreeSet<LocalBindingId> {
        self.roots_by_tuple_path
            .values()
            .flat_map(|sources| &sources.roots)
            .copied()
            .collect()
    }

    fn borrowed(&self) -> Self {
        let mut result = Self::direct(self.roots());
        if let Some(direct) = result.roots_by_tuple_path.get_mut(&[][..]) {
            direct.last_choice = self
                .roots_by_tuple_path
                .values()
                .map(|sources| sources.last_choice)
                .max()
                .unwrap_or(0);
        }
        result
    }
}
