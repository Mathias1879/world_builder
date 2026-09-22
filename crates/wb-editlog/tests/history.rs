mod common;

use common::new_log;
use proptest::prelude::*;
use wb_editlog::{EditError, EditLog, EntityId, TxKind, Value};

fn add(log: &mut EditLog, h: i64) -> EntityId {
    let mut tx = log.transact("add");
    let e = tx.create("test.thing", [("h", Value::Int(h))]);
    tx.commit().unwrap();
    e
}

#[test]
fn undo_redo_basics() {
    let mut log = new_log();
    assert_eq!(log.undo().unwrap_err(), EditError::NothingToUndo);
    assert_eq!(log.redo().unwrap_err(), EditError::NothingToRedo);
    let h0 = log.source_hash();
    let e = add(&mut log, 1);
    let h1 = log.source_hash();
    let mut tx = log.transact("raise");
    tx.set(e, "h", 2i64);
    tx.commit().unwrap();
    let h2 = log.source_hash();

    let u = log.undo().unwrap();
    assert_eq!(log.source_hash(), h1);
    assert!(matches!(log.op(u).unwrap().kind, TxKind::Undo(_)));
    assert_eq!(log.op(u).unwrap().label, "Undo: raise");
    log.undo().unwrap();
    assert_eq!(log.source_hash(), h0);
    assert!(
        log.state().entity(e).is_none(),
        "undoing a create removes the entity"
    );
    assert!(!log.can_undo() && log.can_redo());

    log.redo().unwrap();
    assert_eq!(log.source_hash(), h1);
    log.redo().unwrap();
    assert_eq!(log.source_hash(), h2);
    assert!(!log.can_redo());
}

#[test]
fn new_edit_clears_redo() {
    let mut log = new_log();
    let e = add(&mut log, 1);
    let mut tx = log.transact("raise");
    tx.set(e, "h", 2i64);
    tx.commit().unwrap();
    log.undo().unwrap();
    assert!(log.can_redo());
    let mut tx = log.transact("other");
    tx.set(e, "w", 3i64);
    tx.commit().unwrap();
    assert!(!log.can_redo());
    assert_eq!(log.redo().unwrap_err(), EditError::NothingToRedo);
}

#[test]
fn undo_restores_a_deleted_entity() {
    let mut log = new_log();
    let e = add(&mut log, 1);
    let mut tx = log.transact("del");
    tx.delete(e);
    tx.commit().unwrap();
    log.undo().unwrap();
    assert!(!log.state().entity(e).unwrap().is_deleted());
}

#[derive(Clone, Debug)]
enum Action {
    Add(i64),
    Set(usize, i64),
    Delete(usize),
}

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        (-100i64..100).prop_map(Action::Add),
        (0usize..8, -100i64..100).prop_map(|(i, v)| Action::Set(i, v)),
        (0usize..8).prop_map(Action::Delete),
    ]
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
    #[test]
    fn undoing_everything_returns_to_the_start(actions in prop::collection::vec(action(), 1..24)) {
        let mut log = new_log();
        let h0 = log.source_hash();
        let mut ents: Vec<EntityId> = Vec::new();
        let mut hashes = vec![h0];
        for a in actions {
            let mut tx = log.transact("step");
            match a {
                Action::Add(v) => ents.push(tx.create("test.thing", [("h", Value::Int(v))])),
                Action::Set(i, v) if !ents.is_empty() => { tx.set(ents[i % ents.len()], "h", v); }
                Action::Delete(i) if !ents.is_empty() => { tx.delete(ents[i % ents.len()]); }
                _ => { tx.set(EntityId::PLANET, "noop", 0i64); }
            }
            tx.commit().unwrap();
            hashes.push(log.source_hash());
        }
        for expected in hashes.iter().rev().skip(1) {
            log.undo().unwrap();
            prop_assert_eq!(&log.source_hash(), expected);
        }
        prop_assert_eq!(log.undo().unwrap_err(), EditError::NothingToUndo);
        for expected in hashes.iter().skip(1) {
            log.redo().unwrap();
            prop_assert_eq!(&log.source_hash(), expected);
        }
    }
}
