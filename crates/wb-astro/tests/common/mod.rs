#![allow(dead_code)]
use wb_astro::register_kinds;
use wb_editlog::{ActorId, AuthorId, EditError, EditLog, EntityId, FixedClock, Value};

pub fn new_log() -> EditLog {
    let mut log = EditLog::new(
        ActorId([1; 16]),
        AuthorId([9; 16]),
        Box::new(FixedClock(1_758_000_000_000)),
    );
    register_kinds(&mut log);
    log
}

/// Creates an entity, panicking if it is rejected — for fixtures that must be valid.
pub fn add(log: &mut EditLog, kind: &str, fields: Vec<(&str, Value)>) -> EntityId {
    try_add(log, kind, fields).expect("fixture entity should be valid")
}

/// Creates an entity, returning the validator's verdict — for validator tests.
pub fn try_add(
    log: &mut EditLog,
    kind: &str,
    fields: Vec<(&str, Value)>,
) -> Result<EntityId, EditError> {
    let mut tx = log.transact("add");
    let e = tx.create(kind, fields);
    tx.commit().map(|_| e)
}

/// Sets fields on an existing entity (the planet, usually).
pub fn set(log: &mut EditLog, entity: EntityId, fields: Vec<(&str, Value)>) {
    let mut tx = log.transact("set");
    for (f, v) in fields {
        tx.set(entity, f, v);
    }
    tx.commit().expect("fixture writes should be valid");
}

pub fn f(x: f64) -> Value {
    Value::Float(x)
}

pub fn i(n: i64) -> Value {
    Value::Int(n)
}

pub fn t(s: &str) -> Value {
    Value::from(s)
}

pub fn list(items: &[&str]) -> Value {
    Value::List(items.iter().map(|s| Value::from(*s)).collect())
}
