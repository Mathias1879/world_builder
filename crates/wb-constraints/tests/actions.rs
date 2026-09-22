mod common;

use common::{add, new_log, poly};
use std::collections::BTreeMap;
use wb_constraints::{
    ConstraintError, IssueCode, Param, Preset, Suggestion, apply_suggestion, keep_anyway,
    realism_of, set_realism, unkeep,
};
use wb_editlog::{EditError, EntityId, Value};

fn desert(log: &mut wb_editlog::EditLog) -> EntityId {
    add(
        log,
        "feature.desert",
        vec![
            ("area", poly(&[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)])),
            ("name", Value::from("Sand Sea")),
        ],
    )
}

#[test]
fn suggestions_apply_as_undoable_transactions() {
    let mut log = new_log();
    let d = desert(&mut log);
    let s = Suggestion {
        code: IssueCode::new("lore.set_area_to_drawn"),
        params: [
            ("drawn".to_string(), Param::Float(1234.0)),
            ("unit".to_string(), Param::Text("km2".into())),
        ]
        .into_iter()
        .collect(),
        writes: vec![(d, "name".to_string(), Value::from("Great Sand Sea"))],
    };
    let op = apply_suggestion(&mut log, &s).unwrap();
    assert_eq!(
        log.op(op).unwrap().label,
        "Suggestion: set the stated area to 1234 km2"
    );
    assert_eq!(
        log.state().field(d, "name"),
        Some(&Value::from("Great Sand Sea"))
    );
    log.undo().unwrap();
    assert_eq!(log.state().field(d, "name"), Some(&Value::from("Sand Sea")));

    let ghost = EntityId {
        op: wb_editlog::OpId {
            lamport: 99,
            actor: wb_editlog::ActorId([5; 16]),
        },
        n: 0,
    };
    let bad = Suggestion {
        code: IssueCode::new("x"),
        params: BTreeMap::new(),
        writes: vec![(ghost, "name".into(), Value::from("x"))],
    };
    let before = log.source_hash();
    assert_eq!(
        apply_suggestion(&mut log, &bad).unwrap_err(),
        ConstraintError::UnknownEntity(ghost)
    );
    assert_eq!(log.source_hash(), before);
}

#[test]
fn keep_anyway_and_unkeep() {
    let mut log = new_log();
    let d = desert(&mut log);
    let op = keep_anyway(
        &mut log,
        d,
        &[IssueCode::new("b.two"), IssueCode::new("a.one")],
        Some("Raised by the gods"),
    )
    .unwrap();
    assert_eq!(log.op(op).unwrap().label, "Kept anyway: Sand Sea");
    assert_eq!(
        log.state().field(d, "keep_codes"),
        Some(&Value::List(vec![
            Value::from("a.one"),
            Value::from("b.two")
        ]))
    );
    assert_eq!(
        log.state().field(d, "keep_reason"),
        Some(&Value::from("Raised by the gods"))
    );
    // Re-keeping an already-kept code with no reason writes nothing.
    assert_eq!(
        keep_anyway(&mut log, d, &[IssueCode::new("a.one")], None).unwrap_err(),
        ConstraintError::Edit(EditError::EmptyTransaction)
    );
    unkeep(&mut log, d, &[IssueCode::new("a.one")]).unwrap();
    assert_eq!(
        log.state().field(d, "keep_codes"),
        Some(&Value::List(vec![Value::from("b.two")]))
    );
    unkeep(&mut log, d, &[IssueCode::new("b.two")]).unwrap();
    assert_eq!(log.state().field(d, "keep_codes"), None);
    assert_eq!(log.state().field(d, "keep_reason"), None);
    assert_eq!(
        unkeep(&mut log, d, &[IssueCode::new("b.two")]).unwrap_err(),
        ConstraintError::Edit(EditError::EmptyTransaction)
    );

    let not = add(&mut log, "import.base_map", vec![]);
    assert_eq!(
        keep_anyway(&mut log, not, &[IssueCode::new("x")], None).unwrap_err(),
        ConstraintError::NotAConstraint(not)
    );
}

#[test]
fn actions_refuse_deleted_entities() {
    let mut log = new_log();
    let d = desert(&mut log);
    let mut tx = log.transact("delete");
    tx.delete(d);
    tx.commit().unwrap();

    // A tombstoned entity is still reachable through `State::entity`, so every
    // action must filter it out explicitly.
    assert!(log.state().entity(d).is_some());
    assert!(log.state().entity(d).unwrap().is_deleted());

    let s = Suggestion {
        code: IssueCode::new("x"),
        params: BTreeMap::new(),
        writes: vec![(d, "name".into(), Value::from("Ghost Sea"))],
    };
    let before = log.source_hash();
    assert_eq!(
        apply_suggestion(&mut log, &s).unwrap_err(),
        ConstraintError::UnknownEntity(d)
    );
    assert_eq!(log.source_hash(), before);

    assert_eq!(
        keep_anyway(&mut log, d, &[IssueCode::new("a.one")], Some("why")).unwrap_err(),
        ConstraintError::UnknownEntity(d)
    );
    assert_eq!(
        unkeep(&mut log, d, &[IssueCode::new("a.one")]).unwrap_err(),
        ConstraintError::UnknownEntity(d)
    );
    assert_eq!(log.source_hash(), before);
}

#[test]
fn keep_anyway_writes_nothing_when_unchanged() {
    let mut log = new_log();
    let d = desert(&mut log);

    // No codes and no reason: nothing to write.
    assert_eq!(
        keep_anyway(&mut log, d, &[], None).unwrap_err(),
        ConstraintError::Edit(EditError::EmptyTransaction)
    );
    // Never the empty-list encoding for "nothing kept".
    assert_eq!(log.state().field(d, "keep_codes"), None);

    keep_anyway(&mut log, d, &[IssueCode::new("a.one")], None).unwrap();
    // Already kept, no reason: still nothing to write.
    assert_eq!(
        keep_anyway(&mut log, d, &[IssueCode::new("a.one")], None).unwrap_err(),
        ConstraintError::Edit(EditError::EmptyTransaction)
    );
    // A reason is still writable against an unchanged code set.
    keep_anyway(&mut log, d, &[IssueCode::new("a.one")], Some("by decree")).unwrap();
    assert_eq!(
        log.state().field(d, "keep_reason"),
        Some(&Value::from("by decree"))
    );
    assert_eq!(
        log.state().field(d, "keep_codes"),
        Some(&Value::List(vec![Value::from("a.one")]))
    );
}

#[test]
fn realism_setting() {
    let mut log = new_log();
    set_realism(&mut log, Preset::HighFantasy.value()).unwrap();
    assert_eq!(realism_of(log.state()), 0.85);
    set_realism(&mut log, 7.0).unwrap();
    assert_eq!(realism_of(log.state()), 1.0);
    assert!(matches!(
        set_realism(&mut log, f64::NAN).unwrap_err(),
        ConstraintError::Edit(_)
    ));
    log.undo().unwrap();
    assert_eq!(realism_of(log.state()), 0.85);
}
