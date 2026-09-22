use wb_constraints::{
    CONSTRAINT_KINDS, float, is_constraint_kind, polygon, register_kinds, shape, text,
};
use wb_editlog::{ActorId, AuthorId, EditError, EditLog, FixedClock, Value};
use wb_grid::LatLon;
use wb_world::Geometry;

fn log() -> EditLog {
    let mut log = EditLog::new(ActorId([1; 16]), AuthorId([9; 16]), Box::new(FixedClock(0)));
    register_kinds(&mut log);
    log
}

fn sq() -> Value {
    Value::Geometry(Geometry::Polygon(vec![
        LatLon::from_degrees(0.0, 0.0),
        LatLon::from_degrees(0.0, 1.0),
        LatLon::from_degrees(1.0, 1.0),
    ]))
}

#[test]
fn catalog() {
    assert_eq!(CONSTRAINT_KINDS.len(), 11);
    assert!(is_constraint_kind("lore.area"));
    assert!(!is_constraint_kind("import.base_map"));
}

#[test]
fn valid_entities_commit_and_accessors_read_them() {
    let mut log = log();
    let mut tx = log.transact("forest");
    let f = tx.create(
        "feature.forest",
        [
            ("area", sq()),
            ("biome", Value::from("boreal")),
            ("tolerance", Value::Float(0.2)),
        ],
    );
    tx.commit().unwrap();
    let v = log.state().entity(f).unwrap();
    assert_eq!(polygon(&v, "area").unwrap().len(), 3);
    assert_eq!(text(&v, "biome"), Some("boreal"));
    assert_eq!(float(&v, "tolerance"), Some(0.2));
    let (pts, closed) = shape(&v).unwrap();
    assert_eq!((pts.len(), closed), (3, true));
}

fn rejected(kind: &str, fields: Vec<(&str, Value)>) {
    let mut log = log();
    let mut tx = log.transact("bad");
    tx.create(kind, fields);
    assert!(
        matches!(tx.commit().unwrap_err(), EditError::ValidationFailed { .. }),
        "{kind}"
    );
}

#[test]
fn validators_reject_bad_shapes_and_enums() {
    rejected("feature.forest", vec![]);
    rejected(
        "feature.forest",
        vec![("area", sq()), ("biome", Value::from("jungle"))],
    );
    rejected(
        "feature.river",
        vec![(
            "path",
            Value::Geometry(Geometry::LineString(vec![LatLon::from_degrees(0.0, 0.0)])),
        )],
    );
    rejected(
        "civ.settlement",
        vec![("at", Value::LatLon(LatLon::from_degrees(0.0, 0.0)))],
    );
    rejected(
        "rule.region",
        vec![("area", sq()), ("rule", Value::from("no_gravity"))],
    );
    rejected(
        "lore.area",
        vec![("value", Value::Float(10.0)), ("unit", Value::from("km2"))],
    );
    rejected(
        "lore.area",
        vec![
            ("subject", Value::Entity(wb_editlog::EntityId::PLANET)),
            ("value", Value::Float(-1.0)),
            ("unit", Value::from("km2")),
        ],
    );
    rejected(
        "feature.desert",
        vec![("area", sq()), ("tolerance", Value::Float(1.5))],
    );
    rejected(
        "feature.desert",
        vec![("area", sq()), ("strength", Value::from("firm"))],
    );
    rejected("feature.hills", vec![("peak_m", Value::Float(300.0))]);
}
