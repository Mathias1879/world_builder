mod common;

use common::{add, band, new_log};
use wb_constraints::{
    CONSTRAINT_KINDS, CheckContext, CheckerRegistry, KindCatalog, UnknownTerrain, evaluate, float,
    is_constraint_kind, polygon, register_kinds, shape, text,
};
use wb_editlog::{ActorId, AuthorId, EditError, EditLog, FixedClock, Value};
use wb_grid::LatLon;
use wb_world::{Geometry, Planet};

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

    let mut tx = log.transact("settlement");
    let s = tx.create(
        "civ.settlement",
        [
            ("at", Value::LatLon(LatLon::from_degrees(10.0, 20.0))),
            ("name", Value::from("Kaldros")),
            ("role", Value::from("capital")),
        ],
    );
    tx.commit().unwrap();
    let v = log.state().entity(s).unwrap();
    assert_eq!(text(&v, "role"), Some("capital"));
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
    rejected(
        "civ.settlement",
        vec![
            ("at", Value::LatLon(LatLon::from_degrees(0.0, 0.0))),
            ("name", Value::from("Kaldros")),
            ("role", Value::Float(5.0)),
        ],
    );
}

#[test]
fn starter_catalog_matches_the_const_array() {
    let c = KindCatalog::with_starter_kinds();
    assert_eq!(c.len(), CONSTRAINT_KINDS.len());
    for &k in CONSTRAINT_KINDS {
        assert!(c.contains(k), "{k} missing from the starter catalog");
    }
    assert!(!c.contains("astro.body"));
    // Sorted, so iteration order is part of the contract.
    let listed: Vec<&str> = c.kinds().collect();
    let mut sorted = listed.clone();
    sorted.sort_unstable();
    assert_eq!(listed, sorted);
}

#[test]
fn registering_is_idempotent_and_additive() {
    let mut c = KindCatalog::new();
    assert!(c.is_empty());
    c.register("astro.body");
    c.register("astro.body");
    assert_eq!(c.len(), 1);
    assert!(c.contains("astro.body"));
    assert!(!c.contains("feature.river"));
}

#[test]
fn evaluate_judges_a_registered_kind_and_ignores_an_unregistered_one() {
    let mut log = new_log();
    // `rule.region` is a starter kind; `demo.thing` is not a kind anyone registered.
    add(
        &mut log,
        "rule.region",
        vec![
            ("area", band(10.0, 20.0, -20.0, -10.0)),
            ("rule", Value::from("no_rain")),
        ],
    );
    add(&mut log, "demo.thing", vec![("name", Value::from("x"))]);

    let reg = CheckerRegistry::with_starter_checks();
    let starter = KindCatalog::with_starter_kinds();
    let ctx = CheckContext::with_catalog(log.state(), Planet::default(), &UnknownTerrain, &starter);
    let report = evaluate(&ctx, &reg);
    assert_eq!(report.verdicts.len(), 1);
    assert_eq!(report.verdicts[0].kind, "rule.region");

    let mut wide = KindCatalog::with_starter_kinds();
    wide.register("demo.thing");
    let ctx = CheckContext::with_catalog(log.state(), Planet::default(), &UnknownTerrain, &wide);
    let report = evaluate(&ctx, &reg);
    assert_eq!(report.verdicts.len(), 2);
    // No checker claims `demo.thing`, so it grades clean rather than erroring.
    let v = report
        .verdicts
        .iter()
        .find(|v| v.kind == "demo.thing")
        .expect("demo.thing was registered, so it gets a verdict");
    assert!(v.issues.is_empty());
    assert_eq!(v.score, 1.0);
}

#[test]
fn new_context_uses_the_starter_catalog() {
    let log = new_log();
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    assert!(ctx.catalog.contains("feature.river"));
    assert!(!ctx.catalog.contains("astro.body"));
}
