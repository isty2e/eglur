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
}

thread_local! { static WORK: Cell<Work> = const { Cell::new(Work { samples: 0, roots: 0, snapshots: 0 }) }; }

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
