mod common;

use common::{aethoria, earth_system};
use std::collections::BTreeSet;
use wb_astro::{SkyModel, season_cuts};
use wb_constraints::{Param, apply_suggestion};
use wb_editlog::Value;

#[test]
fn cuts_are_strictly_increasing_and_inside_the_year() {
    let curve: Vec<f64> = (0..100)
        .map(|d| libm::sin(std::f64::consts::TAU * d as f64 / 100.0))
        .collect();
    for n in 1..=12 {
        let cuts = season_cuts(&curve, n);
        assert_eq!(cuts.len(), n, "n = {n}");
        for w in cuts.windows(2) {
            assert!(w[1] > w[0], "n = {n}: {cuts:?} is not increasing");
        }
        assert!(cuts[0] >= 0 && *cuts.last().unwrap() < 100, "{cuts:?}");
    }
}

#[test]
fn more_seasons_than_days_is_capped_and_zero_is_empty() {
    let curve = vec![1.0, 2.0, 3.0];
    assert_eq!(season_cuts(&curve, 0).len(), 0);
    assert_eq!(season_cuts(&curve, 9).len(), 3);
    assert_eq!(season_cuts(&[], 4).len(), 0);
}

#[test]
fn cuts_land_on_the_curves_turning_points() {
    // A clean one-year sine: peak at 25, trough at 75, steepest at 0 and 50.
    let curve: Vec<f64> = (0..100)
        .map(|d| libm::sin(std::f64::consts::TAU * d as f64 / 100.0))
        .collect();
    let cuts = season_cuts(&curve, 4);
    for c in &cuts {
        let near = [0i64, 25, 50, 75]
            .iter()
            .map(|t| (c - t).abs().min(100 - (c - t).abs()))
            .min()
            .unwrap();
        assert!(
            near <= 2,
            "cut at {c} is not near a turning point: {cuts:?}"
        );
    }
}

#[test]
fn four_proposed_seasons_tile_the_aethorian_year_exactly() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let proposals = sky.propose_seasons(4);
    assert_eq!(proposals.len(), 4);

    // Apply them all, then walk every day of the year: exactly one season each.
    let mut log = aethoria();
    for s in &proposals {
        apply_suggestion(&mut log, s).expect("a proposed season must be valid");
    }
    let sky = SkyModel::build(log.state()).unwrap();
    assert_eq!(sky.calendar().seasons.len(), 4);
    for d in 0..390 {
        assert!(
            sky.season_of(d as f64).is_some(),
            "day {d} is in no proposed season"
        );
    }
    // No overlaps: each day's season is the first match, so count distinct runs instead.
    let names: BTreeSet<&str> = (0..390).filter_map(|d| sky.season_of(d as f64)).collect();
    assert_eq!(
        names.len(),
        4,
        "every proposed season should own some days: {names:?}"
    );
}

#[test]
fn proposals_name_their_months_and_days() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let cal_id = sky.calendar().id;
    for s in sky.propose_seasons(3) {
        assert_eq!(s.code.as_str(), "astro.add_season");
        assert!(s.writes.is_empty());
        assert_eq!(s.creates.len(), 1);
        let e = &s.creates[0];
        assert_eq!(e.kind, "astro.season");
        let field = |name: &str| e.fields.iter().find(|(f, _)| f == name).map(|(_, v)| v);
        assert!(matches!(field("name"), Some(Value::Text(_))));
        if let Some(Value::Int(d)) = field("start_day") {
            assert!(*d >= 1);
        } else {
            panic!("start_day missing or not Int");
        }
        if let Some(Value::Int(d)) = field("end_day") {
            assert!(*d >= 1);
        } else {
            panic!("end_day missing or not Int");
        }
        assert!(matches!(field("start_month"), Some(Value::Entity(_))));
        assert!(matches!(field("end_month"), Some(Value::Entity(_))));
        assert_eq!(field("calendar"), Some(&Value::Entity(cal_id)));
        assert!(matches!(s.params.get("name"), Some(Param::Text(_))));
    }
}

#[test]
fn proposed_names_describe_the_light_and_are_distinct() {
    let log = earth_system();
    let sky = SkyModel::build(log.state()).unwrap();
    let names: Vec<String> = sky
        .propose_seasons(4)
        .iter()
        .filter_map(|s| match s.params.get("name") {
            Some(Param::Text(t)) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(names.len(), 4);
    let unique: BTreeSet<&String> = names.iter().collect();
    assert_eq!(unique.len(), 4, "names must be distinct: {names:?}");
    // Earth at 45°N has a clear high and low season.
    assert!(
        names.iter().any(|n| n.contains("High Sun")),
        "no bright season among {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("Low Sun")),
        "no dark season among {names:?}"
    );
}

#[test]
fn one_proposed_season_covers_the_whole_year() {
    let mut log = aethoria();
    let proposals = SkyModel::build(log.state()).unwrap().propose_seasons(1);
    assert_eq!(proposals.len(), 1);
    apply_suggestion(&mut log, &proposals[0]).unwrap();
    let sky = SkyModel::build(log.state()).unwrap();
    for d in [0, 1, 195, 389] {
        assert!(sky.season_of(d as f64).is_some(), "day {d}");
    }
}

#[test]
fn a_world_with_no_seasonal_variation_still_gets_a_tiling() {
    // Zero tilt: the curve is flat, so there are no turning points to find.
    let mut log = aethoria();
    common::set(
        &mut log,
        wb_editlog::EntityId::PLANET,
        vec![
            ("axial_tilt_deg", common::f(0.0)),
            ("eccentricity", common::f(0.0)),
        ],
    );
    let sky = SkyModel::build(log.state()).unwrap();
    let proposals = sky.propose_seasons(4);
    assert_eq!(proposals.len(), 4);
    let mut applied = log;
    for s in &proposals {
        apply_suggestion(&mut applied, s).unwrap();
    }
    let sky = SkyModel::build(applied.state()).unwrap();
    for d in 0..390 {
        assert!(sky.season_of(d as f64).is_some(), "day {d} uncovered");
    }
}
