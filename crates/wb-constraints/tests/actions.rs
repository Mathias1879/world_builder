mod common;

use common::{add, band, new_log, poly};
use std::collections::BTreeMap;
use wb_constraints::{
    ConstraintError, IssueCode, NewEntity, Param, Preset, Suggestion, apply_suggestion,
    keep_anyway, realism_of, set_realism, unkeep,
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
        creates: Vec::new(),
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
        creates: Vec::new(),
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

    // `kept()` now accepts any live entity that has a kind at all (subsystems register
    // their own kinds in a `KindCatalog` this function never sees), so `NotAConstraint`
    // is retargeted at an entity with no kind whatsoever. The ordinary `Transaction` API
    // cannot produce one — `create` always sets `kind`, and the planet's is fixed — so
    // this borrows the sync path's raw-op technique (as in `wb-editlog`'s own sync
    // tests): clone an already-committed op, retarget its one write at a fresh entity id
    // with a non-reserved field, and feed it in via `apply_ops`, which applies writes
    // directly with no "known entity" or kind-schema check at all.
    let source = log.op(d.op).unwrap().clone();
    let name_write = source
        .writes
        .iter()
        .find(|w| w.field.as_str() == "name")
        .unwrap()
        .clone();
    let no_kind = EntityId {
        op: wb_editlog::OpId {
            lamport: log.heads().iter().map(|h| h.lamport).max().unwrap() + 1,
            actor: wb_editlog::ActorId([9; 16]),
        },
        n: 0,
    };
    let mut raw = source;
    raw.id = no_kind.op;
    raw.parents = log.heads().clone();
    raw.writes = vec![wb_editlog::FieldWrite {
        entity: no_kind,
        field: name_write.field,
        value: Value::from("Nameless"),
    }];
    log.apply_ops(vec![raw]).unwrap();
    assert_eq!(log.state().entity(no_kind).unwrap().kind(), None);
    assert_eq!(
        keep_anyway(&mut log, no_kind, &[IssueCode::new("x")], None).unwrap_err(),
        ConstraintError::NotAConstraint(no_kind)
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
        creates: Vec::new(),
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
    // NaN is not clamped: `f64::clamp` propagates it and value canonicalization
    // rejects it, so nothing is written.
    assert!(matches!(
        set_realism(&mut log, f64::NAN).unwrap_err(),
        ConstraintError::Edit(EditError::InvalidValue { .. })
    ));
    log.undo().unwrap();
    assert_eq!(realism_of(log.state()), 0.85);
}

#[test]
fn applying_a_suggestion_creates_entities() {
    let mut log = new_log();
    let before = log.state().len();
    let s = Suggestion {
        code: IssueCode::new("demo.add_two"),
        params: BTreeMap::new(),
        writes: Vec::new(),
        creates: vec![
            NewEntity {
                kind: "civ.settlement".to_string(),
                fields: vec![
                    ("at".to_string(), Value::LatLon(common::d(10.0, 20.0))),
                    ("name".to_string(), Value::from("Kaldros")),
                ],
            },
            NewEntity {
                kind: "civ.settlement".to_string(),
                fields: vec![
                    ("at".to_string(), Value::LatLon(common::d(12.0, 22.0))),
                    ("name".to_string(), Value::from("Thanewick")),
                ],
            },
        ],
    };
    apply_suggestion(&mut log, &s).unwrap();

    assert_eq!(log.state().len(), before + 2);
    let names: Vec<&str> = log
        .state()
        .live()
        .filter(|v| v.kind() == Some("civ.settlement"))
        .filter_map(|v| match v.get("name") {
            Some(Value::Text(t)) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["Kaldros", "Thanewick"]);
}

#[test]
fn creating_an_invalid_entity_commits_nothing() {
    let mut log = new_log();
    let before = log.state().len();
    // `civ.settlement` requires `at` and `name`; this one has neither.
    let s = Suggestion {
        code: IssueCode::new("demo.bad"),
        params: BTreeMap::new(),
        writes: Vec::new(),
        creates: vec![NewEntity {
            kind: "civ.settlement".to_string(),
            fields: vec![("role".to_string(), Value::from("capital"))],
        }],
    };
    assert!(apply_suggestion(&mut log, &s).is_err());
    assert_eq!(log.state().len(), before);
}

#[test]
fn a_suggestion_can_create_and_write_in_one_transaction() {
    let mut log = new_log();
    let lake = add(
        &mut log,
        "feature.lake",
        vec![("area", band(10.0, 12.0, -12.0, -10.0))],
    );
    let s = Suggestion {
        code: IssueCode::new("demo.both"),
        params: BTreeMap::new(),
        writes: vec![(lake, "name".to_string(), Value::from("Mirrormere"))],
        creates: vec![NewEntity {
            kind: "civ.settlement".to_string(),
            fields: vec![
                ("at".to_string(), Value::LatLon(common::d(11.0, -11.0))),
                ("name".to_string(), Value::from("Shorehold")),
            ],
        }],
    };
    let op = apply_suggestion(&mut log, &s).unwrap();
    assert_eq!(
        log.state().field(lake, "name"),
        Some(&Value::from("Mirrormere"))
    );
    assert_eq!(log.op(op).map(|o| o.writes.len()), Some(4));
}
