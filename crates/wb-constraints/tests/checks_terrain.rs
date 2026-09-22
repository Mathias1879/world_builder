mod common;

use common::{WestIsLand, add, line, new_log, poly};
use wb_constraints::checks::{CoastCheck, RiverMouthCheck};
use wb_constraints::{CheckContext, Checker, UnknownTerrain};
use wb_editlog::Value;
use wb_world::Planet;

fn codes(fs: &[wb_constraints::Finding]) -> Vec<(&str, f64)> {
    fs.iter().map(|f| (f.code.as_str(), f.badness)).collect()
}

#[test]
fn coast_measures_ocean_fraction() {
    let mut log = new_log();
    let inland = add(
        &mut log,
        "feature.forest",
        vec![(
            "area",
            poly(&[(0.0, -20.0), (0.0, -10.0), (10.0, -10.0), (10.0, -20.0)]),
        )],
    );
    let straddling = add(
        &mut log,
        "feature.desert",
        vec![(
            "area",
            poly(&[(0.0, -10.0), (0.0, 10.0), (10.0, 10.0), (10.0, -10.0)]),
        )],
    );
    let at_sea = add(
        &mut log,
        "civ.settlement",
        vec![
            ("at", Value::LatLon(common::d(0.0, 30.0))),
            ("name", Value::from("Drowned")),
        ],
    );
    let lake = add(
        &mut log,
        "feature.lake",
        vec![("area", poly(&[(0.0, 10.0), (0.0, 20.0), (5.0, 20.0)]))],
    );
    let ctx = CheckContext::new(log.state(), Planet::default(), &WestIsLand);
    let s = log.state();
    assert!(CoastCheck.check(s.entity(inland).unwrap(), &ctx).is_empty());
    let f = CoastCheck.check(s.entity(straddling).unwrap(), &ctx);
    assert_eq!(f.len(), 1);
    assert!(f[0].badness > 0.2 && f[0].badness < 0.8, "{}", f[0].badness);
    assert_eq!(
        codes(&CoastCheck.check(s.entity(at_sea).unwrap(), &ctx)),
        vec![("coast.over_ocean", 1.0)]
    );
    assert!(
        CoastCheck.check(s.entity(lake).unwrap(), &ctx).is_empty(),
        "lakes are exempt"
    );

    let unknown = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    assert!(
        CoastCheck
            .check(s.entity(at_sea).unwrap(), &unknown)
            .is_empty()
    );
}

#[test]
fn river_mouth_and_source() {
    let mut log = new_log();
    let to_sea = add(
        &mut log,
        "feature.river",
        vec![("path", line(&[(0.0, -20.0), (0.0, 5.0)]))],
    );
    let inland = add(
        &mut log,
        "feature.river",
        vec![("path", line(&[(0.0, -20.0), (0.0, -5.0)]))],
    );
    let from_sea = add(
        &mut log,
        "feature.river",
        vec![("path", line(&[(0.0, 20.0), (0.0, -5.0)]))],
    );
    add(
        &mut log,
        "feature.lake",
        vec![(
            "area",
            poly(&[(-1.0, -6.0), (-1.0, -4.0), (1.0, -4.0), (1.0, -6.0)]),
        )],
    );
    let into_nothing = add(
        &mut log,
        "feature.river",
        vec![("path", line(&[(10.0, -20.0), (10.0, -5.0)]))],
    );
    let ctx = CheckContext::new(log.state(), Planet::default(), &WestIsLand);
    let s = log.state();
    assert!(
        RiverMouthCheck
            .check(s.entity(to_sea).unwrap(), &ctx)
            .is_empty()
    );
    assert!(
        RiverMouthCheck
            .check(s.entity(inland).unwrap(), &ctx)
            .is_empty(),
        "ends in the lake"
    );
    assert_eq!(
        codes(&RiverMouthCheck.check(s.entity(from_sea).unwrap(), &ctx)),
        vec![("river.source_in_ocean", 0.8)]
    );
    assert_eq!(
        codes(&RiverMouthCheck.check(s.entity(into_nothing).unwrap(), &ctx)),
        vec![("river.inland_mouth", 0.3)]
    );
}
