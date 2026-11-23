use std::cell::Cell;
use std::collections::BTreeSet;

use eglur_hir::lower;
use eglur_source::{FileId, SourceFile};
use eglur_syntax::{FrontendOutcome, parse};

use super::*;
use crate::ModuleId;

#[derive(Clone, Copy, Debug, Default)]
struct Work {
    samples: usize,
    roots: usize,
    snapshots: usize,
    parked_reads: usize,
    shared_entries: usize,
}

thread_local! { static WORK: Cell<Work> = const { Cell::new(Work { samples: 0, roots: 0, snapshots: 0, parked_reads: 0, shared_entries: 0 }) }; }

pub(super) fn observe_shared_entry() {
    WORK.update(|mut w| {
        w.shared_entries += 1;
        w
    });
}

pub(super) fn observe_parked_read() {
    WORK.update(|mut w| {
        w.parked_reads += 1;
        w
    });
}

pub(super) fn observe_sample(value: &BindingState) {
    WORK.update(|mut w| {
        w.samples += 1;
        w.roots += value.provenance.roots().len();
        w
    });
}

pub(super) fn observe_snapshot() {
    WORK.update(|mut w| {
        w.snapshots += 1;
        w
    });
}

fn owner() -> ExpressionKey {
    let source = SourceFile::new(FileId::new(0), b"fn f() -> Unit = ()".to_vec());
    let FrontendOutcome::Parsed(parsed) = parse(&source) else {
        panic!("fixture must parse")
    };
    let hir = lower(&parsed);
    let (_, body) = hir.bodies().next().unwrap();
    let module = ModuleId::from_index(0).unwrap();
    ExpressionKey::new(module, body.expressions().next().unwrap().0)
}

#[test]
fn sparse_normal_exits_match_dense_constructor_snapshots() {
    let owner = owner();
    let binding =
        |i| LocalBindingId::from_index(owner.module(), owner.expression().body(), i).unwrap();
    let inputs = [binding(0), binding(1)];
    let roots = [binding(2), binding(3), binding(4)];
    // (matched constructors, guard write to p, selected body write to q, normal body)
    let cases = [
        ([true, false], Some(1), None, true),
        ([false, true], Some(2), Some(0), true),
        ([true, true], None, Some(2), true),
        ([true, true], Some(0), Some(1), false),
        ([true, false], None, None, false),
        ([false, true], None, None, true),
    ];
    for mut plan in 0..cases.len().pow(4) {
        let mut state = FlowState::default();
        let mut paths = MatchPaths::begin(&mut state, owner, true);
        let mut search = [[None; 2]; 2];
        let mut outputs = [BTreeSet::new(), BTreeSet::new()];
        let mut normal = false;
        for _ in 0..4 {
            let (mask, guard, body, continues) = cases[plan % cases.len()];
            plan /= cases.len();
            paths.enter(&mut state, Some(ConstructorSet(mask)));
            if let Some(i) = guard {
                state.set_alias(
                    inputs[0],
                    ValueProvenance::direct(BTreeSet::from([roots[i]])),
                );
            }
            assert!(paths.guard_false(&mut state, GuardScope::new(1), |_| Some(true)));
            if let Some(i) = body {
                state.set_alias(
                    inputs[1],
                    ValueProvenance::direct(BTreeSet::from([roots[i]])),
                );
            }
            paths.finish_arm(&mut state, continues);

            for (lane, selected) in search.iter_mut().zip(mask) {
                if !selected {
                    continue;
                }
                if let Some(i) = guard {
                    lane[0] = Some(roots[i]);
                }
                if continues {
                    let mut snapshot = *lane;
                    if let Some(i) = body {
                        snapshot[1] = Some(roots[i]);
                    }
                    for (out, value) in outputs.iter_mut().zip(snapshot) {
                        out.extend(value);
                    }
                    normal = true;
                }
            }
        }
        assert_eq!(paths.finish(&mut state), normal);
        if normal {
            for (id, expected) in inputs.into_iter().zip(outputs) {
                assert_eq!(
                    state
                        .alias(id)
                        .map(ValueProvenance::roots)
                        .unwrap_or_default(),
                    expected
                );
            }
        }
    }
}

#[test]
fn growing_false_prefix_does_not_revisit_every_binding_at_every_normal_exit() {
    let owner = owner();
    let binding =
        |i| LocalBindingId::from_index(owner.module(), owner.expression().body(), i).unwrap();
    for width in [8, 32, 128, 512] {
        WORK.set(Work::default());
        let mut state = FlowState::default();
        let mut paths = MatchPaths::begin(&mut state, owner, true);
        for i in 0..width {
            paths.enter(&mut state, Some(ConstructorSet([true, false])));
            state.set_alias(
                binding(i),
                ValueProvenance::direct(BTreeSet::from([binding(width + i)])),
            );
            assert!(paths.guard_false(&mut state, GuardScope::new(1), |_| Some(true)));
            paths.finish_arm(&mut state, true);
        }
        paths.enter(&mut state, Some(ConstructorSet([false, true])));
        paths.finish_arm(&mut state, true);
        assert!(paths.finish(&mut state));
        for i in 0..width {
            assert_eq!(
                state.alias(binding(i)).unwrap().roots(),
                BTreeSet::from([binding(width + i)])
            );
        }
        let w = WORK.get();
        // Per binding: one unchanged prefix and at most two constructor suffixes.
        assert!(
            w.samples <= 3 * width && w.roots <= w.samples,
            "{width}: {w:?}"
        );
        assert_eq!(w.snapshots, 0);
        println!("width={width} {w:?}");
    }
}

#[test]
fn shared_bodies_only_project_the_bindings_they_use() {
    let owner = owner();
    let id = |i| LocalBindingId::from_index(owner.module(), owner.expression().body(), i).unwrap();
    let first = Some(ConstructorSet([true, false]));
    let both = Some(ConstructorSet([true, true]));
    for width in [8, 128, 512] {
        for arms in [4, 32] {
            let (a, b, c, local) = (id(width), id(width + 1), id(width + 2), id(width + 3));
            let source = |r| ValueProvenance::direct(BTreeSet::from([r]));
            let merged = |r| ValueProvenance::direct(BTreeSet::from([a, r]));
            let mut state = FlowState::default();
            for i in 0..width {
                state.set_alias(id(i), source(a));
            }
            let mut outer = MatchPaths::begin(&mut state, owner, true);
            outer.enter(&mut state, first);
            for i in 0..width {
                state.set_alias(id(i), source(b));
            }
            assert!(outer.guard_false(&mut state, GuardScope::new(1), |_| Some(true)));
            outer.finish_arm(&mut state, false);

            WORK.set(Work::default());
            for i in 0..arms {
                let p = id(i % width);
                outer.enter(&mut state, both);
                let routed = state.alias(p).unwrap().clone();
                assert_eq!(routed.for_constructor(0), source(b));
                assert_eq!(routed.for_constructor(1), source(a));
                state.set_alias(local, routed);
                assert!(outer.guard_false(&mut state, GuardScope::new(2), |id| Some(id != local)));
                outer.select_body(&mut state);
                assert_eq!(state.alias(local), Some(&merged(b)));

                let mut child = MatchPaths::begin(&mut state, owner, true);
                child.enter(&mut state, first);
                assert_eq!(state.alias(p), Some(&merged(b)));
                assert_eq!(state.alias(local), Some(&merged(b)));
                child.finish_arm(&mut state, true);
                assert!(child.finish(&mut state));
                outer.finish_arm(&mut state, false);
            }
            let work = WORK.get();
            assert!(work.shared_entries <= 2 * arms, "{width}/{arms}: {work:?}");
            assert_eq!(work.snapshots, 0);

            outer.enter(&mut state, first);
            state.set_alias(id(0), source(c));
            assert!(outer.guard_false(&mut state, GuardScope::new(3), |_| Some(true)));
            outer.finish_arm(&mut state, false);
            outer.enter(&mut state, both);
            assert_eq!(state.alias(id(0)).unwrap().for_constructor(0), source(c));
            outer.select_body(&mut state);
            assert_eq!(state.alias(id(0)), Some(&merged(c)));
            outer.finish_arm(&mut state, true);
            assert!(outer.finish(&mut state));
        }
    }
}

#[test]
fn deep_reads_go_directly_to_the_parked_writer_and_only_changes_travel_back() {
    let owner = owner();
    let binding =
        |i| LocalBindingId::from_index(owner.module(), owner.expression().body(), i).unwrap();
    let sources = |i| ValueProvenance::direct(BTreeSet::from([binding(i)]));
    let one = Some(ConstructorSet([true, false]));

    for depth in [1, 8, 32] {
        for width in [2, 16, 64] {
            for changed in [false, true] {
                let mut state = FlowState::default();
                for i in 0..width {
                    state.set_alias(binding(i), sources(width));
                }
                let mut outer = MatchPaths::begin(&mut state, owner, true);
                outer.enter(&mut state, one);
                for i in 0..width / 2 {
                    state.set_alias(binding(i), sources(width + 1));
                }
                assert!(outer.guard_false(&mut state, GuardScope::new(1), |_| Some(true)));
                outer.select_body(&mut state);
                for i in width / 2..width {
                    state.set_alias(binding(i), sources(width + 1));
                }

                WORK.set(Work::default());
                let mut searches = Vec::new();
                for _ in 0..depth {
                    let nested = MatchPaths::begin(&mut state, owner, true);
                    nested.enter(&mut state, one);
                    nested.select_body(&mut state);
                    searches.push(nested);
                }
                for i in 0..width {
                    assert_eq!(state.alias(binding(i)), Some(&sources(width + 1)));
                }
                assert!(WORK.get().parked_reads <= width);
                if changed {
                    for i in 0..width {
                        state.set_alias(binding(i), sources(width + 2));
                    }
                }
                for mut nested in searches.into_iter().rev() {
                    nested.finish_arm(&mut state, true);
                    assert!(nested.finish(&mut state));
                }
                let work = WORK.get();
                assert!(work.samples <= if changed { width * depth } else { 0 });
                assert_eq!(work.snapshots, 0);

                outer.finish_arm(&mut state, true);
                assert!(outer.finish(&mut state));
                for i in 0..width {
                    assert_eq!(
                        state.alias(binding(i)),
                        Some(&sources(width + 1 + usize::from(changed)))
                    );
                }
            }
        }
    }
}

#[test]
fn an_explicit_default_shadows_an_ancestor_and_rewinding_restores_it() {
    let owner = owner();
    let binding =
        |i| LocalBindingId::from_index(owner.module(), owner.expression().body(), i).unwrap();
    let p = binding(0);
    let referent = ValueProvenance::direct(BTreeSet::from([binding(1)]));
    let one = Some(ConstructorSet([true, false]));
    let mut state = FlowState::default();
    state.set_alias(p, referent.clone());

    let mut outer = MatchPaths::begin(&mut state, owner, true);
    outer.enter(&mut state, one);
    outer.select_body(&mut state);
    let mut branches = Alternatives::begin(&mut state);
    let mut inner = MatchPaths::begin(&mut state, owner, true);
    inner.enter(&mut state, one);
    state.end_binding(p);
    inner.finish_arm(&mut state, true);
    assert!(inner.finish(&mut state));

    let mut sibling = MatchPaths::begin(&mut state, owner, true);
    sibling.enter(&mut state, one);
    assert!(state.alias(p).is_none_or(ValueProvenance::is_empty));
    assert!(state.exit_snapshot().alias(p).is_none());
    sibling.finish_arm(&mut state, true);
    assert!(sibling.finish(&mut state));
    branches.finish_arm(&mut state, false);
    branches.unchanged_path();
    assert!(branches.finish(&mut state));

    let mut sibling = MatchPaths::begin(&mut state, owner, true);
    sibling.enter(&mut state, one);
    assert_eq!(state.alias(p), Some(&referent));
    sibling.finish_arm(&mut state, true);
    assert!(sibling.finish(&mut state));
    outer.finish_arm(&mut state, true);
    assert!(outer.finish(&mut state));
}
