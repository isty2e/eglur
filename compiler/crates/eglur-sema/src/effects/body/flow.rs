use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use super::{LocalBindingId, MutableRoot};

type Roots = BTreeSet<MutableRoot>;
type Aliases = BTreeMap<LocalBindingId, Roots>;

#[derive(Default)]
pub(super) struct FlowState {
    aliases: Aliases,
    escaped: BTreeSet<LocalBindingId>,
    forks: Vec<Fork>,
}

struct Fork {
    arm_count: usize,
    finished: usize,
    arm: ArmChanges,
    aliases_at_exit: BTreeMap<LocalBindingId, AliasContribution>,
    escaped_at_exit: BTreeSet<LocalBindingId>,
}

#[derive(Default)]
struct ArmChanges {
    aliases: BTreeMap<LocalBindingId, AliasUndo>,
    escaped: BTreeSet<LocalBindingId>,
}

enum AliasUndo {
    Replaced(Option<Roots>),
    Added { was_absent: bool, roots: Roots },
}

struct AliasContribution {
    replacing_arms: usize,
    roots: Roots,
}

impl FlowState {
    pub(super) fn alias_roots(&self, binding: LocalBindingId) -> Option<&Roots> {
        self.aliases.get(&binding)
    }

    pub(super) fn rebind(&mut self, binding: LocalBindingId, roots: Roots) {
        let previous = self.aliases.insert(binding, roots);
        let Some(fork) = self.forks.last_mut() else {
            return;
        };

        match fork.arm.aliases.entry(binding) {
            Entry::Vacant(slot) => {
                slot.insert(AliasUndo::Replaced(previous));
            }
            Entry::Occupied(mut slot) => {
                // A nested join may already have extended the value we just displaced.
                if let AliasUndo::Added { was_absent, roots } = slot.get() {
                    let entry = if *was_absent {
                        None
                    } else {
                        let mut entry = previous.expect("an extended alias has a live value");
                        remove_additions(&mut entry, roots);
                        Some(entry)
                    };
                    slot.insert(AliasUndo::Replaced(entry));
                }
            }
        }
    }

    fn extend_alias(&mut self, binding: LocalBindingId, roots: Roots) {
        if roots.is_empty() {
            return;
        }
        let (live, was_absent) = match self.aliases.entry(binding) {
            Entry::Vacant(slot) => (slot.insert(Roots::new()), true),
            Entry::Occupied(slot) => (slot.into_mut(), false),
        };

        for root in roots {
            if live.insert(root.clone())
                && let Some(fork) = self.forks.last_mut()
            {
                let undo = fork
                    .arm
                    .aliases
                    .entry(binding)
                    .or_insert_with(|| AliasUndo::Added {
                        was_absent,
                        roots: Roots::new(),
                    });
                if let AliasUndo::Added { roots, .. } = undo {
                    roots.insert(root);
                }
            }
        }
    }

    pub(super) fn mark_escaping(&mut self, roots: impl IntoIterator<Item = MutableRoot>) {
        for root in roots {
            if let MutableRoot::Local(binding) = root {
                self.escape(binding);
            }
        }
    }

    fn escape(&mut self, binding: LocalBindingId) {
        if self.escaped.insert(binding)
            && let Some(fork) = self.forks.last_mut()
        {
            fork.arm.escaped.insert(binding);
        }
    }

    pub(super) fn account_for_escape(&self, root: MutableRoot) -> MutableRoot {
        match root {
            MutableRoot::Local(binding) if self.escaped.contains(&binding) => MutableRoot::Nonlocal,
            other => other,
        }
    }

    pub(super) fn begin_fork(&mut self, arm_count: usize) {
        debug_assert_ne!(arm_count, 0);
        self.forks.push(Fork {
            arm_count,
            finished: 0,
            arm: ArmChanges::default(),
            aliases_at_exit: BTreeMap::new(),
            escaped_at_exit: BTreeSet::new(),
        });
    }

    pub(super) fn finish_arm(&mut self) {
        let fork = self
            .forks
            .last_mut()
            .expect("branch belongs to an open fork");
        debug_assert!(fork.finished < fork.arm_count);
        fork.finished += 1;
        let arm = std::mem::take(&mut fork.arm);

        for (binding, undo) in arm.aliases {
            let contribution = undo.rollback(binding, &mut self.aliases);
            match fork.aliases_at_exit.entry(binding) {
                Entry::Vacant(slot) => {
                    slot.insert(contribution);
                }
                Entry::Occupied(mut slot) => {
                    let joined = slot.get_mut();
                    joined.replacing_arms += contribution.replacing_arms;
                    for root in contribution.roots {
                        joined.roots.insert(root);
                    }
                }
            }
        }
        for binding in arm.escaped {
            self.escaped.remove(&binding);
            fork.escaped_at_exit.insert(binding);
        }
    }

    pub(super) fn finish_fork(&mut self) {
        let fork = self.forks.pop().expect("join belongs to an open fork");
        debug_assert_eq!(fork.finished, fork.arm_count);

        // Apply through the enclosing arm so it can still restore its own entry.
        for (binding, exit) in fork.aliases_at_exit {
            if exit.replacing_arms == fork.arm_count {
                self.rebind(binding, exit.roots);
            } else {
                self.extend_alias(binding, exit.roots);
            }
        }
        for binding in fork.escaped_at_exit {
            self.escape(binding);
        }
    }
}

impl AliasUndo {
    fn rollback(self, binding: LocalBindingId, live: &mut Aliases) -> AliasContribution {
        match self {
            Self::Replaced(entry) => {
                let exit = match entry {
                    Some(roots) => live.insert(binding, roots),
                    None => live.remove(&binding),
                };
                AliasContribution {
                    replacing_arms: 1,
                    roots: exit.expect("a rebound alias has an exit value"),
                }
            }
            Self::Added { was_absent, roots } => {
                if was_absent {
                    live.remove(&binding);
                } else {
                    remove_additions(
                        live.get_mut(&binding).expect("extended alias exists"),
                        &roots,
                    );
                }
                AliasContribution {
                    replacing_arms: 0,
                    roots,
                }
            }
        }
    }
}

fn remove_additions(live: &mut Roots, added: &Roots) {
    for root in added {
        let removed = live.remove(root);
        debug_assert!(removed);
    }
}
