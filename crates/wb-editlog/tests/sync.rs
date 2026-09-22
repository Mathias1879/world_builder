mod common;

use common::{ACTOR_A, ACTOR_B, new_log_as};
use proptest::prelude::*;
use std::collections::BTreeSet;
use wb_editlog::{EditError, EditLog, EntityId, Op, Value};

fn seeded_pair() -> (EditLog, EditLog, EntityId) {
    let mut a = new_log_as(ACTOR_A);
    let mut tx = a.transact("seed");
    let e = tx.create("test.thing", [("h", Value::Int(0))]);
    tx.commit().unwrap();
    let mut b = new_log_as(ACTOR_B);
    b.apply_ops(a.ops_since(&BTreeSet::new())).unwrap();
    (a, b, e)
}

#[test]
fn ops_since_and_apply_ops() {
    let (a, mut b, e) = seeded_pair();
    assert_eq!(
        b.state_at(a.heads()).unwrap().field(e, "h"),
        Some(&Value::Int(0))
    );
    assert!(a.ops_since(a.heads()).is_empty());
    assert_eq!(
        b.apply_ops(a.ops_since(&BTreeSet::new())).unwrap_err(),
        EditError::DuplicateOp(e.op)
    );
}

#[test]
fn apply_ops_rejects_malformed_input_atomically() {
    let (a, mut b, e) = seeded_pair();
    let mut orphan: Op = a.op(e.op).unwrap().clone();
    orphan.id.lamport = 50;
    let ghost = wb_editlog::OpId {
        lamport: 40,
        actor: ACTOR_B,
    };
    orphan.parents = [ghost].into_iter().collect();
    assert_eq!(
        b.apply_ops(vec![orphan.clone()]).unwrap_err(),
        EditError::UnknownParent(ghost)
    );

    let mut backwards = orphan.clone();
    backwards.parents = [e.op].into_iter().collect();
    backwards.id.lamport = 1;
    backwards.id.actor = ACTOR_B;
    assert!(matches!(
        b.apply_ops(vec![backwards]).unwrap_err(),
        EditError::CorruptFile { .. }
    ));

    let mut nan = orphan.clone();
    nan.parents = [e.op].into_iter().collect();
    nan.writes[0].value = Value::Float(f64::NAN);
    assert!(matches!(
        b.apply_ops(vec![nan]).unwrap_err(),
        EditError::CorruptFile { .. }
    ));

    let mut empty = orphan;
    empty.parents = [e.op].into_iter().collect();
    empty.writes.clear();
    assert!(matches!(
        b.apply_ops(vec![empty]).unwrap_err(),
        EditError::CorruptFile { .. }
    ));
    assert_eq!(
        b.ops_since(&BTreeSet::new()).len(),
        1,
        "nothing partial was applied"
    );
}

fn cfg() -> ProptestConfig {
    ProptestConfig {
        cases: 64,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(cfg())]
    /// Two actors edit concurrently; whichever order a third log receives the ops in,
    /// the merged state is identical.
    #[test]
    fn concurrent_edits_converge(
        edits_a in prop::collection::vec((0u8..3, -50i64..50), 1..8),
        edits_b in prop::collection::vec((0u8..3, -50i64..50), 1..8),
    ) {
        let (mut a, mut b, e) = seeded_pair();
        for (f, v) in edits_a {
            let mut tx = a.transact("a");
            tx.set(e, ["h", "w", "d"][f as usize], v);
            tx.commit().unwrap();
        }
        for (f, v) in edits_b {
            let mut tx = b.transact("b");
            tx.set(e, ["h", "w", "d"][f as usize], v);
            tx.commit().unwrap();
        }
        let base: BTreeSet<_> = [e.op].into_iter().collect();
        let only_a = a.ops_since(&base);
        let only_b = b.ops_since(&base);
        let merged_heads: BTreeSet<_> = a.heads().union(b.heads()).copied().collect();

        let mut x = new_log_as(wb_editlog::ActorId([3; 16]));
        x.apply_ops(a.ops_since(&BTreeSet::new())).unwrap();
        x.apply_ops(only_b.clone()).unwrap();
        let mut y = new_log_as(wb_editlog::ActorId([4; 16]));
        y.apply_ops(b.ops_since(&BTreeSet::new())).unwrap();
        y.apply_ops(only_a).unwrap();

        let sx = x.state_at(&merged_heads).unwrap();
        let sy = y.state_at(&merged_heads).unwrap();
        prop_assert_eq!(sx.source_hash(), sy.source_hash());
        prop_assert_eq!(sx, sy);
    }
}

#[test]
fn applied_ops_extend_the_current_branch_only() {
    let (a, mut b, e) = seeded_pair();
    assert_eq!(b.heads(), a.heads(), "b's main branch now ends at the seed");
    b.fork("side", wb_editlog::ForkFrom::Current).unwrap();
    let mut a = a;
    let mut tx = a.transact("a2");
    tx.set(e, "h", 5i64);
    tx.commit().unwrap();
    b.apply_ops(a.ops_since(&[e.op].into_iter().collect()))
        .unwrap();
    assert_eq!(b.heads(), a.heads());
    assert_eq!(b.state().field(e, "h"), Some(&Value::Int(5)));
    assert_eq!(
        b.branch("side")
            .unwrap()
            .heads
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![e.op],
        "other branches untouched"
    );
    let mut tx = b.transact("b edits after receiving");
    tx.set(e, "w", 1i64);
    let id = tx.commit().unwrap();
    assert_eq!(b.op(id).unwrap().parents, *a.heads());
}
