use std::cell::Cell;

use eglur_source::{ByteOffset, ByteRange, FileId, FileSpan};

use super::*;
use crate::{BoundaryRootKey, DiagnosticPhase, PrimarySpan, SourceLabel};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Work {
    inputs: usize,
    pool_checks: usize,
    foreign: usize,
    roots: usize,
    finishes: usize,
    prepared: usize,
    snapshots: usize,
    snapshot_roots: usize,
}

thread_local! {
    static WORK: Cell<Work> = Cell::new(Work::default());
}

pub(super) enum Step {
    Input,
    PoolCheck,
    Foreign,
    Root,
    Finish(usize),
    Snapshot(usize),
}

pub(super) fn observe(step: Step) {
    WORK.update(|mut work| {
        match step {
            Step::Input => work.inputs += 1,
            Step::PoolCheck => work.pool_checks += 1,
            Step::Foreign => work.foreign += 1,
            Step::Root => work.roots += 1,
            Step::Finish(count) => {
                work.finishes += 1;
                work.prepared += count;
            }
            Step::Snapshot(count) => {
                work.snapshots += 1;
                work.snapshot_roots += count;
            }
        }
        work
    });
}

fn roots(store: &mut DiagnosticStore, count: usize) -> Vec<Poison> {
    (0..count)
        .map(|index| {
            let diagnostic = Diagnostic::new(
                DiagnosticCode::new(format!("ROOT_{index:03}")).unwrap(),
                Severity::Error,
                DiagnosticPhase::new("type").unwrap(),
                "independent failure",
                PrimarySpan::Unavailable,
            );
            let root = DiagnosticRoot::boundary(
                BoundaryRootKey::new(format!("root-{index}")).unwrap(),
                diagnostic,
            )
            .unwrap();
            store.record_failure(DiagnosticFailure::Root(root)).unwrap()
        })
        .collect()
}

fn subset(roots: &[Poison], mask: usize) -> Poison {
    let mut selected = roots
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .map(|(_, root)| root);
    let mut result = selected.next().unwrap().clone();
    for root in selected {
        result = result.merge(root).unwrap();
    }
    result
}

#[test]
fn equality_uses_root_membership_and_store_identity() {
    let local = roots(&mut DiagnosticStore::new(), 3);
    let foreign = roots(&mut DiagnosticStore::new(), 3);
    let pair = local[0].merge(&local[2]).unwrap();
    assert!(pair.same_roots(&local[2].merge(&local[0]).unwrap()));
    assert!(!pair.same_roots(&local[0]));
    assert!(!pair.same_roots(&subset(&foreign, 0b101)));
}

fn pending_roots(accumulated: &PoisonAccumulator) -> Vec<usize> {
    match &accumulated.pending {
        PendingRoots::Empty => Vec::new(),
        PendingRoots::Shared(poison) => poison.roots().to_vec(),
        PendingRoots::Union { roots, .. } => roots.iter().copied().collect(),
    }
}

fn shared_snapshot(poison: &Poison) -> &Arc<SharedRootSnapshot> {
    let RootSnapshot::Shared(snapshot) = &poison.snapshot else {
        panic!("multi-root test provenance must have shared storage");
    };
    snapshot
}

#[test]
fn empty_accumulation_cannot_create_poison() {
    WORK.set(Work::default());
    assert!(PoisonAccumulator::new().finish().is_none());
    assert!(PoisonAccumulator::default().finish().is_none());
    assert_eq!(WORK.get(), Work::default());
}

#[test]
fn distinct_inputs_materialize_only_the_final_union_in_either_order() {
    for count in [8, 32, 128] {
        let mut store = DiagnosticStore::new();
        let inputs = roots(&mut store, count);
        for reverse in [false, true] {
            WORK.set(Work::default());
            let mut accumulated = PoisonAccumulator::new();
            for offset in 0..count {
                let index = if reverse { count - offset - 1 } else { offset };
                accumulated.include(&inputs[index]).unwrap();
                assert_eq!(pending_roots(&accumulated).len(), offset + 1);
                assert_eq!(WORK.get().prepared, 0);
            }
            let result = accumulated.finish().unwrap();
            assert_eq!(result.roots(), (0..count).collect::<Vec<_>>());
            assert!(Arc::ptr_eq(&result.store, &store.identity));
            assert_eq!(
                WORK.get(),
                Work {
                    inputs: count,
                    pool_checks: count - 1,
                    roots: count,
                    finishes: 1,
                    prepared: count,
                    snapshots: 1,
                    snapshot_roots: count,
                    ..Work::default()
                }
            );
            for (index, input) in inputs.iter().enumerate() {
                assert_eq!(input.roots(), [index]);
            }
        }
    }
}

#[test]
fn repeated_provenance_forwards_one_snapshot_without_visiting_roots() {
    for count in [8, 32, 128] {
        let mut store = DiagnosticStore::new();
        let inputs = roots(&mut store, count);
        let complete = inputs
            .iter()
            .skip(1)
            .try_fold(inputs[0].clone(), |left, right| left.merge(right))
            .unwrap();

        for input in [&inputs[0], &complete] {
            WORK.set(Work::default());
            let mut accumulated = PoisonAccumulator::new();
            for _ in 0..count {
                accumulated.include(input).unwrap();
                assert_eq!(pending_roots(&accumulated).len(), input.roots().len());
                assert_eq!(WORK.get().prepared, 0);
            }
            let result = accumulated.finish().unwrap();
            assert_eq!(result.roots(), input.roots());
            if input.roots().len() > 1 {
                assert!(Arc::ptr_eq(
                    shared_snapshot(&result),
                    shared_snapshot(input)
                ));
            }
            assert_eq!(
                WORK.get(),
                Work {
                    inputs: count,
                    pool_checks: count - 1,
                    finishes: 1,
                    ..Work::default()
                }
            );
        }
    }
}

#[test]
fn every_small_pair_of_subsets_matches_an_independent_membership_oracle() {
    let mut store = DiagnosticStore::new();
    let inputs = roots(&mut store, 5);
    for left in 1..32 {
        for right in 1..32 {
            let expected = (0..5)
                .filter(|index| (left | right) & (1 << index) != 0)
                .collect::<Vec<_>>();
            let left = subset(&inputs, left);
            let right = subset(&inputs, right);
            let merged = left.merge(&right).unwrap();
            let mut accumulated = PoisonAccumulator::new();
            accumulated.include(&left).unwrap();
            accumulated.include(&right).unwrap();
            let result = accumulated.finish().unwrap();
            assert_eq!(result.roots(), expected);
            assert_eq!(merged.roots(), expected);
            assert!(Arc::ptr_eq(&result.store, &merged.store));
        }
    }
}

#[test]
fn cloning_and_pairwise_self_union_keep_the_immutable_snapshot() {
    let mut store = DiagnosticStore::new();
    let inputs = roots(&mut store, 128);
    let mut accumulated = PoisonAccumulator::new();
    for input in &inputs {
        accumulated.include(input).unwrap();
    }
    let original = accumulated.finish().unwrap();
    let expected = original.roots().to_vec();

    WORK.set(Work::default());
    for _ in 0..128 {
        let cloned = original.clone();
        let merged = cloned.merge(&original).unwrap();
        assert!(Arc::ptr_eq(
            shared_snapshot(&cloned),
            shared_snapshot(&original)
        ));
        assert!(Arc::ptr_eq(
            shared_snapshot(&merged),
            shared_snapshot(&original)
        ));
        assert_eq!(merged.roots(), expected);
    }
    assert_eq!(WORK.get(), Work::default());
    assert_eq!(format!("{original:?}"), "Poison { root_count: 128, .. }");

    drop(store);
    assert_eq!(original.roots(), expected);
}

#[test]
fn singleton_folds_preserve_sharing_through_nested_completion() {
    let mut store = DiagnosticStore::new();
    let inputs = roots(&mut store, 128);
    let original = subset(&inputs[..5], 31);
    let mut current = original.clone();

    WORK.set(Work::default());
    for _ in 0..128 {
        let mut accumulated = PoisonAccumulator::new();
        accumulated.include(&current).unwrap();
        current = accumulated.finish().unwrap();
        assert!(Arc::ptr_eq(
            shared_snapshot(&current),
            shared_snapshot(&original)
        ));
    }
    assert_eq!(
        WORK.get(),
        Work {
            inputs: 128,
            finishes: 128,
            ..Work::default()
        }
    );
    assert_eq!(current.roots(), [0, 1, 2, 3, 4]);
}

#[test]
fn interleaved_shared_inputs_are_each_scanned_only_once() {
    for count in [8, 32, 128] {
        let mut store = DiagnosticStore::new();
        let _inputs = roots(&mut store, 2 * count);
        let left = Poison::from_roots(
            Arc::clone(&store.identity),
            (0..count).collect::<Vec<_>>().into_boxed_slice(),
        );
        let right = Poison::from_roots(
            Arc::clone(&store.identity),
            (count / 2..count + count / 2)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );

        WORK.set(Work::default());
        let mut accumulated = PoisonAccumulator::new();
        for _ in 0..count {
            accumulated.include(&left.clone()).unwrap();
            accumulated.include(&right.clone()).unwrap();
        }
        let result = accumulated.finish().unwrap();
        let width = count + count / 2;
        assert_eq!(result.roots(), (0..width).collect::<Vec<_>>());
        assert_eq!(
            WORK.get(),
            Work {
                inputs: 2 * count,
                pool_checks: 2 * count - 1,
                roots: 2 * count,
                finishes: 1,
                prepared: width,
                snapshots: 1,
                snapshot_roots: width,
                ..Work::default()
            }
        );
    }
}

#[test]
fn equal_independent_snapshots_keep_complete_membership_without_global_interning() {
    let mut store = DiagnosticStore::new();
    let inputs = roots(&mut store, 5);
    let first = subset(&inputs, 31);
    let second = subset(&inputs, 31);
    assert!(!Arc::ptr_eq(
        shared_snapshot(&first),
        shared_snapshot(&second)
    ));

    WORK.set(Work::default());
    let mut accumulated = PoisonAccumulator::new();
    accumulated.include(&first).unwrap();
    accumulated.include(&second).unwrap();
    let result = accumulated.finish().unwrap();
    assert_eq!(result.roots(), first.roots());
    assert_eq!(WORK.get().roots, 10);
    assert_eq!(WORK.get().prepared, 5);
}

#[test]
fn union_identity_witnesses_do_not_retain_temporary_root_payloads() {
    let mut store = DiagnosticStore::new();
    let inputs = roots(&mut store, 5);
    let mut accumulated = PoisonAccumulator::new();
    let mut witnesses = Vec::new();
    for mask in 1..32 {
        if usize::count_ones(mask) < 2 {
            continue;
        }
        let selected = subset(&inputs, mask);
        let input = Poison::from_roots(
            Arc::clone(&store.identity),
            selected.roots().to_vec().into_boxed_slice(),
        );
        witnesses.push(Arc::downgrade(shared_snapshot(&input)));
        accumulated.include(&input).unwrap();
    }
    assert!(witnesses.iter().all(|witness| witness.upgrade().is_none()));
    let PendingRoots::Union { seen, .. } = &accumulated.pending else {
        panic!("multiple snapshots must form a union");
    };
    assert_eq!(seen.len(), witnesses.len());
    assert!(seen.values().all(|witness| witness.strong_count() == 0));
    for witness in &witnesses {
        let retained = seen.get(&witness.as_ptr().addr()).unwrap();
        assert!(Weak::ptr_eq(retained, witness));
    }

    let result = accumulated.finish().unwrap();
    assert_eq!(result.roots(), [0, 1, 2, 3, 4]);
}

#[test]
fn store_validation_precedes_even_a_matching_snapshot_identity() {
    let mut local = DiagnosticStore::new();
    let other = DiagnosticStore::new();
    let inputs = roots(&mut local, 2);
    for first in [inputs[0].clone(), inputs[0].merge(&inputs[1]).unwrap()] {
        let foreign = Poison {
            store: Arc::clone(&other.identity),
            snapshot: first.snapshot.clone(),
        };
        assert_eq!(
            first.merge(&foreign).unwrap_err(),
            DiagnosticStoreError::ForeignPoison
        );
        for multiple in [false, true] {
            let mut accumulated = PoisonAccumulator::new();
            accumulated.include(&first).unwrap();
            if multiple {
                accumulated.include(&inputs[1]).unwrap();
            }
            let before = pending_roots(&accumulated);

            WORK.set(Work::default());
            assert_eq!(
                accumulated.include(&foreign),
                Err(DiagnosticStoreError::ForeignPoison)
            );
            assert_eq!(pending_roots(&accumulated), before);
            assert_eq!(
                WORK.get(),
                Work {
                    inputs: 1,
                    pool_checks: 1,
                    foreign: 1,
                    ..Work::default()
                }
            );
            accumulated.include(&inputs[1]).unwrap();
            assert_eq!(accumulated.finish().unwrap().roots(), [0, 1]);
        }
    }
}

#[test]
fn single_roots_need_neither_shared_payloads_nor_union_identity_witnesses() {
    let mut store = DiagnosticStore::new();
    let inputs = roots(&mut store, 128);
    let mut accumulated = PoisonAccumulator::new();
    for (index, input) in inputs.iter().enumerate() {
        assert!(matches!(input.snapshot, RootSnapshot::Single(root) if root == index));
        accumulated.include(input).unwrap();
    }
    let PendingRoots::Union { seen, .. } = &accumulated.pending else {
        panic!("distinct single roots must form a union");
    };
    assert!(seen.is_empty());
    let result = accumulated.finish().unwrap();
    assert_eq!(result.roots(), (0..128).collect::<Vec<_>>());

    let normalized = Poison::from_roots(Arc::clone(&store.identity), Box::new([7]));
    assert!(matches!(normalized.snapshot, RootSnapshot::Single(7)));
    store
        .record_failure(DiagnosticFailure::Derivative(normalized))
        .unwrap();
}

#[test]
fn shared_provenance_and_pending_unions_remain_send_and_sync() {
    fn require_send_sync<T: Send + Sync>(_: &T) {}

    let mut store = DiagnosticStore::new();
    let inputs = roots(&mut store, 3);
    let mut accumulated = PoisonAccumulator::new();
    accumulated.include(&inputs[0]).unwrap();
    accumulated.include(&inputs[2]).unwrap();
    require_send_sync(&accumulated);
    require_send_sync(&inputs[0]);

    let result = std::thread::spawn(move || {
        accumulated.include(&inputs[1]).unwrap();
        accumulated.finish().unwrap()
    })
    .join()
    .unwrap();
    assert_eq!(result.roots(), [0, 1, 2]);
    store
        .record_failure(DiagnosticFailure::Derivative(result))
        .unwrap();
}

#[test]
fn foreign_input_is_rejected_before_pending_state_changes() {
    let mut local = DiagnosticStore::new();
    let mut other = DiagnosticStore::new();
    let inputs = roots(&mut local, 3);
    let foreign = roots(&mut other, 3);
    let foreign = subset(&foreign, 7);
    let mut accumulated = PoisonAccumulator::new();
    accumulated.include(&inputs[0]).unwrap();
    accumulated.include(&inputs[2]).unwrap();
    let before = pending_roots(&accumulated);

    WORK.set(Work::default());
    assert_eq!(
        accumulated.include(&foreign),
        Err(DiagnosticStoreError::ForeignPoison)
    );
    assert_eq!(pending_roots(&accumulated), before);
    let PendingRoots::Union { store, .. } = &accumulated.pending else {
        panic!("distinct inputs must preserve their pending union");
    };
    assert!(Arc::ptr_eq(store, &local.identity));
    assert_eq!(
        WORK.get(),
        Work {
            inputs: 1,
            pool_checks: 1,
            foreign: 1,
            ..Work::default()
        }
    );

    accumulated.include(&inputs[1]).unwrap();
    let result = accumulated.finish().unwrap();
    assert_eq!(result.roots(), [0, 1, 2]);
    local
        .record_failure(DiagnosticFailure::Derivative(result.clone()))
        .unwrap();
    assert_eq!(
        other
            .record_failure(DiagnosticFailure::Derivative(result))
            .unwrap_err(),
        DiagnosticStoreError::ForeignPoison
    );
}

#[test]
fn first_input_establishes_identity_without_rebinding_previous_results() {
    let mut first = DiagnosticStore::new();
    let mut second = DiagnosticStore::new();
    let first_root = roots(&mut first, 1).pop().unwrap();
    let second_root = roots(&mut second, 1).pop().unwrap();
    assert_eq!(first_root.roots(), second_root.roots());

    let mut accumulated = PoisonAccumulator::new();
    accumulated.include(&first_root).unwrap();
    let first_result = accumulated.finish().unwrap();
    let mut accumulated = PoisonAccumulator::new();
    accumulated.include(&second_root).unwrap();
    assert_eq!(
        accumulated.include(&first_result),
        Err(DiagnosticStoreError::ForeignPoison)
    );
    let second_result = accumulated.finish().unwrap();
    assert!(Arc::ptr_eq(&first_result.store, &first.identity));
    assert!(Arc::ptr_eq(&second_result.store, &second.identity));

    drop(first);
    assert_eq!(first_result.merge(&first_root).unwrap().roots(), [0]);
    assert_eq!(
        first_result.merge(&second_result).unwrap_err(),
        DiagnosticStoreError::ForeignPoison
    );
}

fn source_root(store: &mut DiagnosticStore, position: u32) -> Poison {
    let span = SourceSpan::byte_only(
        SourceLabel::new("main.eglur").unwrap(),
        FileSpan::new(
            FileId::new(1),
            ByteRange::new(ByteOffset::new(position), ByteOffset::new(position + 1)).unwrap(),
        ),
    );
    let diagnostic = Diagnostic::new(
        DiagnosticCode::new("NAME_NOT_FOUND").unwrap(),
        Severity::Error,
        DiagnosticPhase::new("resolve").unwrap(),
        "missing name",
        PrimarySpan::Source(span),
    );
    store
        .record_failure(DiagnosticFailure::Root(
            DiagnosticRoot::source(diagnostic).unwrap(),
        ))
        .unwrap()
}

#[test]
fn suppression_does_not_change_accumulated_or_prior_provenance() {
    let mut store = DiagnosticStore::new();
    let first = source_root(&mut store, 20);
    let checkpoint = store.checkpoint();
    let second = source_root(&mut store, 5);
    let third = source_root(&mut store, 10);
    let expected = first.merge(&second).unwrap().merge(&third).unwrap();
    let prior = expected.clone();

    let mut accumulated = PoisonAccumulator::new();
    accumulated.include(&third).unwrap();
    accumulated.include(&first).unwrap();
    store.select_preceding_roots(&first, &second).unwrap();
    accumulated.include(&second).unwrap();
    let result = accumulated.finish().unwrap();
    assert_eq!(result.roots(), expected.roots());
    assert_eq!(prior.roots(), [0, 1, 2]);
    store
        .record_failure(DiagnosticFailure::Derivative(result))
        .unwrap();

    let construct = SourceSpan::byte_only(
        SourceLabel::new("main.eglur").unwrap(),
        FileSpan::new(
            FileId::new(1),
            ByteRange::new(ByteOffset::new(0), ByteOffset::new(30)).unwrap(),
        ),
    );
    assert_eq!(
        store
            .roots_since_within(&checkpoint, &construct)
            .unwrap()
            .unwrap()
            .roots(),
        [1, 2]
    );
    let DiagnosticSnapshot::Rejected(snapshot) = store.finalize(ErrorLimit::new(1)) else {
        panic!("independent retained roots must reject compilation");
    };
    assert_eq!(snapshot.diagnostics().len(), 1);
    assert_eq!(snapshot.omitted_error_count(), 1);
    assert_eq!(
        snapshot
            .first_error()
            .primary_span()
            .as_source()
            .unwrap()
            .file_span()
            .range()
            .start(),
        ByteOffset::new(10)
    );
}
