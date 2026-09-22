mod common;

use common::{AUTHOR, T0, new_log};
use wb_editlog::{EditError, EntityId, EntityKind, EntityView, TxKind, Value};

#[test]
fn new_log_is_just_the_planet_on_main() {
    let log = new_log();
    assert_eq!(log.current_branch(), "main");
    assert!(log.heads().is_empty());
    assert_eq!(log.state().len(), 1);
}

#[test]
fn commit_creates_entities_and_advances_heads() {
    let mut log = new_log();
    let mut tx = log.transact("Drew mountain range");
    let range = tx.create("feature.mountain_range", [("peak_m", Value::Float(5000.0))]);
    let hills = tx.create("feature.hills", []);
    tx.set(EntityId::PLANET, "axial_tilt_deg", 23.4);
    let id = tx.commit().unwrap();

    assert_eq!(range.op, id);
    assert_eq!((range.n, hills.n), (0, 1));
    assert_eq!(log.heads().iter().copied().collect::<Vec<_>>(), vec![id]);
    let e = log.state().entity(range).unwrap();
    assert_eq!(e.kind(), Some("feature.mountain_range"));
    assert_eq!(e.get("peak_m"), Some(&Value::Float(5000.0)));
    assert_eq!(
        log.state().field(EntityId::PLANET, "axial_tilt_deg"),
        Some(&Value::Float(23.4))
    );
    let op = log.op(id).unwrap();
    assert_eq!(
        (op.author, op.time_ms, &op.kind, op.label.as_str()),
        (AUTHOR, T0, &TxKind::Edit, "Drew mountain range")
    );
    assert!(op.parents.is_empty());

    let mut tx = log.transact("Raise peak");
    tx.set(range, "peak_m", 6000.0);
    let id2 = tx.commit().unwrap();
    assert!(id2 > id);
    assert_eq!(
        log.op(id2)
            .unwrap()
            .parents
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![id]
    );
}

#[test]
fn commit_rejections_leave_the_log_unchanged() {
    let mut log = new_log();
    let before = log.source_hash();

    assert_eq!(
        log.transact("nothing").commit().unwrap_err(),
        EditError::EmptyTransaction
    );

    let mut tx = log.transact("bad key");
    tx.set(EntityId::PLANET, "Bad Key", 1i64);
    assert!(matches!(tx.commit().unwrap_err(), EditError::InvalidKey(_)));

    let mut tx = log.transact("reserved");
    tx.set(EntityId::PLANET, "kind", "x");
    assert!(matches!(tx.commit().unwrap_err(), EditError::InvalidKey(_)));

    let mut tx = log.transact("nan");
    tx.set(EntityId::PLANET, "tilt", f64::NAN);
    assert!(matches!(
        tx.commit().unwrap_err(),
        EditError::InvalidValue { .. }
    ));

    let mut tx = log.transact("kill planet");
    tx.delete(EntityId::PLANET);
    assert!(matches!(
        tx.commit().unwrap_err(),
        EditError::InvalidValue { .. }
    ));

    let ghost = EntityId {
        op: wb_editlog::OpId {
            lamport: 99,
            actor: wb_editlog::ActorId([5; 16]),
        },
        n: 0,
    };
    let mut tx = log.transact("ghost");
    tx.set(ghost, "x", 1i64);
    assert_eq!(tx.commit().unwrap_err(), EditError::UnknownEntity(ghost));

    let long = "x".repeat(201);
    let mut tx = log.transact(&long);
    tx.set(EntityId::PLANET, "tilt", 1.0);
    assert!(matches!(
        tx.commit().unwrap_err(),
        EditError::InvalidValue { .. }
    ));

    assert_eq!(log.source_hash(), before);
    assert!(log.heads().is_empty());
}

#[test]
fn validators_reject_atomically() {
    let mut log = new_log();
    log.register_validator(
        EntityKind::new("feature.mountain_range").unwrap(),
        Box::new(|e: EntityView<'_>| match e.get("peak_m") {
            Some(Value::Float(h)) if *h > 0.0 => Ok(()),
            _ => Err("peak_m must be a positive float".to_string()),
        }),
    );
    let mut tx = log.transact("bad range");
    tx.create("feature.mountain_range", [("peak_m", Value::Float(-1.0))]);
    let err = tx.commit().unwrap_err();
    assert!(
        matches!(err, EditError::ValidationFailed { ref kind, .. } if kind == "feature.mountain_range")
    );
    assert!(log.heads().is_empty());

    let mut tx = log.transact("good range");
    let e = tx.create("feature.mountain_range", [("peak_m", Value::Float(4000.0))]);
    tx.commit().unwrap();
    let mut tx = log.transact("break it");
    tx.set(e, "peak_m", "tall");
    assert!(matches!(
        tx.commit().unwrap_err(),
        EditError::ValidationFailed { .. }
    ));
    assert_eq!(log.state().field(e, "peak_m"), Some(&Value::Float(4000.0)));

    // Deleted entities are not validated.
    let mut tx = log.transact("delete");
    tx.delete(e);
    tx.commit().unwrap();
    assert!(log.state().entity(e).unwrap().is_deleted());
}

#[test]
fn delete_and_undelete() {
    let mut log = new_log();
    let mut tx = log.transact("add");
    let e = tx.create("test.thing", []);
    tx.commit().unwrap();
    let mut tx = log.transact("del");
    tx.delete(e);
    tx.commit().unwrap();
    assert_eq!(log.state().live().count(), 1);
    let mut tx = log.transact("undel");
    tx.undelete(e);
    tx.commit().unwrap();
    assert_eq!(log.state().live().count(), 2);
}

#[test]
fn assets_are_content_addressed() {
    let mut log = new_log();
    let a = log.add_asset("image/png", vec![1, 2, 3]);
    let b = log.add_asset("image/png", vec![1, 2, 3]);
    assert_eq!(a, b);
    assert_eq!(a.0, *blake3::hash(&[1, 2, 3]).as_bytes());
    assert_eq!(log.asset(a), Some(&[1u8, 2, 3][..]));
    let mut tx = log.import("Imported base map");
    let map = tx.create("import.base_map", [("image", Value::Asset(a))]);
    let id = tx.commit().unwrap();
    assert_eq!(log.op(id).unwrap().kind, TxKind::Import);
    assert_eq!(log.state().field(map, "image"), Some(&Value::Asset(a)));
}

#[test]
fn state_at_rejects_unknown_heads() {
    let log = new_log();
    let ghost = wb_editlog::OpId {
        lamport: 3,
        actor: wb_editlog::ActorId([4; 16]),
    };
    assert_eq!(
        log.state_at(&[ghost].into_iter().collect()).unwrap_err(),
        EditError::UnknownParent(ghost)
    );
}

#[test]
fn title_and_author_names() {
    let mut log = new_log();
    log.set_title("Aethoria");
    log.set_author_name("Matthew");
    assert_eq!(log.title(), "Aethoria");
}
