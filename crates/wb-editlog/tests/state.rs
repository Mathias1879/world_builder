use std::collections::{BTreeMap, BTreeSet};
use wb_editlog::{
    ActorId, AuthorId, DELETED, EntityId, FieldKey, FieldWrite, KIND, Op, OpId, State, TxKind,
    Value, materialize,
};

fn id(lamport: u64, actor: u8) -> OpId {
    OpId {
        lamport,
        actor: ActorId([actor; 16]),
    }
}
fn ent(o: OpId) -> EntityId {
    EntityId { op: o, n: 0 }
}
fn w(e: EntityId, f: &str, v: Value) -> FieldWrite {
    FieldWrite {
        entity: e,
        field: FieldKey::new(f).unwrap(),
        value: v,
    }
}
fn op(o: OpId, parents: &[OpId], writes: Vec<FieldWrite>) -> Op {
    Op {
        id: o,
        parents: parents.iter().copied().collect(),
        author: AuthorId([7; 16]),
        time_ms: 0,
        kind: TxKind::Edit,
        label: "t".into(),
        writes,
    }
}
fn graph(ops: Vec<Op>) -> BTreeMap<OpId, Op> {
    ops.into_iter().map(|o| (o.id, o)).collect()
}
fn heads(h: &[OpId]) -> BTreeSet<OpId> {
    h.iter().copied().collect()
}

#[test]
fn new_state_has_only_the_planet() {
    let s = State::new();
    assert_eq!(s.len(), 1);
    assert_eq!(s.entity(EntityId::PLANET).unwrap().kind(), Some("planet"));
    assert_eq!(State::default(), s);
}

#[test]
fn highest_op_id_wins_for_concurrent_writes() {
    let base = id(1, 1);
    let e = ent(base);
    let ops = graph(vec![
        op(
            base,
            &[],
            vec![w(e, KIND, "test.thing".into()), w(e, "h", 1i64.into())],
        ),
        op(id(2, 1), &[base], vec![w(e, "h", 10i64.into())]),
        op(id(2, 2), &[base], vec![w(e, "h", 20i64.into())]),
    ]);
    let s = materialize(&ops, &heads(&[id(2, 1), id(2, 2)]));
    assert_eq!(s.field(e, "h"), Some(&Value::Int(20)));
    let only_a = materialize(&ops, &heads(&[id(2, 1)]));
    assert_eq!(only_a.field(e, "h"), Some(&Value::Int(10)));
}

#[test]
fn null_removes_fields_and_empty_entities() {
    let a = id(1, 1);
    let e = ent(a);
    let ops = graph(vec![
        op(
            a,
            &[],
            vec![w(e, KIND, "test.thing".into()), w(e, "h", 1i64.into())],
        ),
        op(id(2, 1), &[a], vec![w(e, "h", Value::Null)]),
        op(id(3, 1), &[id(2, 1)], vec![w(e, KIND, Value::Null)]),
    ]);
    let s2 = materialize(&ops, &heads(&[id(2, 1)]));
    assert_eq!(s2.field(e, "h"), None);
    assert!(s2.entity(e).is_some());
    let s3 = materialize(&ops, &heads(&[id(3, 1)]));
    assert!(s3.entity(e).is_none());
}

#[test]
fn deleted_entities_stay_but_are_not_live() {
    let a = id(1, 1);
    let e = ent(a);
    let ops = graph(vec![
        op(a, &[], vec![w(e, KIND, "test.thing".into())]),
        op(id(2, 1), &[a], vec![w(e, DELETED, true.into())]),
    ]);
    let s = materialize(&ops, &heads(&[id(2, 1)]));
    assert!(s.entity(e).unwrap().is_deleted());
    assert_eq!(s.entities().count(), 2);
    assert_eq!(s.live().count(), 1);
}

#[test]
fn planet_kind_cannot_change() {
    let a = id(1, 1);
    let ops = graph(vec![op(
        a,
        &[],
        vec![
            w(EntityId::PLANET, KIND, "not.planet".into()),
            w(EntityId::PLANET, "tilt", 23.4.into()),
        ],
    )]);
    let s = materialize(&ops, &heads(&[a]));
    let p = s.entity(EntityId::PLANET).unwrap();
    assert_eq!(p.kind(), Some("planet"));
    assert_eq!(p.get("tilt"), Some(&Value::Float(23.4)));
}

#[test]
fn source_hash_depends_on_state_not_history() {
    let a = id(1, 1);
    let e = ent(a);
    let direct = graph(vec![op(
        a,
        &[],
        vec![w(e, KIND, "test.thing".into()), w(e, "h", 5i64.into())],
    )]);
    let roundabout = graph(vec![
        op(
            a,
            &[],
            vec![w(e, KIND, "test.thing".into()), w(e, "h", 1i64.into())],
        ),
        op(id(2, 1), &[a], vec![w(e, "h", 9i64.into())]),
        op(id(3, 1), &[id(2, 1)], vec![w(e, "h", 5i64.into())]),
    ]);
    let h1 = materialize(&direct, &heads(&[a])).source_hash();
    let h2 = materialize(&roundabout, &heads(&[id(3, 1)])).source_hash();
    assert_eq!(h1, h2);
    let h3 = materialize(&roundabout, &heads(&[id(2, 1)])).source_hash();
    assert_ne!(h1, h3);
}
