mod common;

use common::{add, new_log, poly};
use wb_constraints::checks::{LatitudeSanityCheck, RegionRulesCheck};
use wb_constraints::{CheckContext, Checker, CheckerRegistry, UnknownTerrain};
use wb_editlog::Value;
use wb_world::Planet;

fn box_at(lat: f64, lon: f64) -> Value {
    poly(&[
        (lat - 1.0, lon - 1.0),
        (lat - 1.0, lon + 1.0),
        (lat + 1.0, lon + 1.0),
        (lat + 1.0, lon - 1.0),
    ])
}

#[test]
fn region_rules_flag_forbidden_features() {
    let mut log = new_log();
    let dry = add(
        &mut log,
        "rule.region",
        vec![
            (
                "area",
                poly(&[(-10.0, -10.0), (-10.0, 10.0), (10.0, 10.0), (10.0, -10.0)]),
            ),
            ("rule", Value::from("no_rain")),
        ],
    );
    let lake = add(&mut log, "feature.lake", vec![("area", box_at(0.0, 0.0))]);
    add(&mut log, "feature.desert", vec![("area", box_at(0.0, 3.0))]);
    add(
        &mut log,
        "feature.forest",
        vec![("area", box_at(40.0, 40.0))],
    );
    let storm = add(
        &mut log,
        "rule.region",
        vec![
            ("area", box_at(0.0, 0.0)),
            ("rule", Value::from("perpetual_storm")),
        ],
    );
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let f = RegionRulesCheck.check(log.state().entity(dry).unwrap(), &ctx);
    assert_eq!(f.len(), 1);
    assert_eq!(
        (f[0].code.as_str(), f[0].badness),
        ("rule.forbidden_feature", 0.6)
    );
    assert_eq!(f[0].related, vec![lake]);
    assert!(
        RegionRulesCheck
            .check(log.state().entity(storm).unwrap(), &ctx)
            .is_empty()
    );
}

#[test]
fn latitude_sanity() {
    let mut log = new_log();
    let tropical_glacier = add(
        &mut log,
        "feature.glacier",
        vec![("area", box_at(5.0, 0.0))],
    );
    let high_glacier = add(
        &mut log,
        "feature.glacier",
        vec![
            ("area", box_at(5.0, 0.0)),
            ("elevation_m", Value::Float(5200.0)),
        ],
    );
    let tropical_north = add(
        &mut log,
        "feature.forest",
        vec![
            ("area", box_at(50.0, 0.0)),
            ("biome", Value::from("tropical")),
        ],
    );
    let boreal_south = add(
        &mut log,
        "feature.forest",
        vec![
            ("area", box_at(-10.0, 0.0)),
            ("biome", Value::from("boreal")),
        ],
    );
    let fine = add(
        &mut log,
        "feature.forest",
        vec![
            ("area", box_at(60.0, 0.0)),
            ("biome", Value::from("boreal")),
        ],
    );
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let s = log.state();
    let code = |e| {
        LatitudeSanityCheck
            .check(s.entity(e).unwrap(), &ctx)
            .first()
            .map(|f| (f.code.0.clone(), f.badness))
    };
    assert_eq!(
        code(tropical_glacier),
        Some(("climate.lowland_tropical_glacier".into(), 0.6))
    );
    assert_eq!(code(high_glacier), None);
    assert_eq!(
        code(tropical_north),
        Some(("climate.tropical_forest_high_latitude".into(), 0.5))
    );
    assert_eq!(
        code(boreal_south),
        Some(("climate.boreal_forest_low_latitude".into(), 0.4))
    );
    assert_eq!(code(fine), None);
}

#[test]
fn starter_registry() {
    let r = CheckerRegistry::with_starter_checks();
    assert_eq!(
        r.ids(),
        vec![
            "static.coast",
            "static.latitude_sanity",
            "static.lore_area",
            "static.region_rules",
            "static.relative_position",
            "static.river_mouth"
        ]
    );
}
