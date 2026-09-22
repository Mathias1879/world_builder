mod common;

use common::{add, band, new_log};
use wb_constraints::checks::{LoreAreaCheck, RelativePositionCheck};
use wb_constraints::{CheckContext, Checker, FindingKind, Param, UnknownTerrain};
use wb_editlog::Value;
use wb_world::Planet;

#[test]
fn lore_area_compares_drawn_and_stated() {
    let mut log = new_log();
    // ≈ 223 000 km² per degree of longitude between 15°N and 35°N.
    let sea = add(
        &mut log,
        "feature.desert",
        vec![("area", band(15.0, 35.0, 30.0, 47.0))],
    );
    let wrong = add(
        &mut log,
        "lore.area",
        vec![
            ("subject", Value::Entity(sea)),
            ("value", Value::Float(600_000.0)),
            ("unit", Value::from("mi2")),
        ],
    );
    let right = add(
        &mut log,
        "lore.area",
        vec![
            ("subject", Value::Entity(sea)),
            ("value", Value::Float(3_790_000.0)),
            ("unit", Value::from("km2")),
        ],
    );
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let s = log.state();

    let f = LoreAreaCheck.check(s.entity(wrong).unwrap(), &ctx);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].code.as_str(), "lore.area_mismatch");
    let Param::Float(ratio) = f[0].params["ratio"] else {
        panic!()
    };
    assert!(ratio > 2.3 && ratio < 2.6, "ratio {ratio}");
    assert!((f[0].badness - (ratio - 1.15) / 1.85).abs() < 1e-12);
    let sug = &f[0].suggestions[0];
    assert_eq!(sug.code.as_str(), "lore.set_area_to_drawn");
    assert_eq!(sug.writes[0].0, wrong);
    assert_eq!(sug.writes[0].1, "value");

    let ok = LoreAreaCheck.check(s.entity(right).unwrap(), &ctx);
    assert_eq!(ok.len(), 1);
    assert_eq!(ok[0].kind, FindingKind::Support);
}

#[test]
fn relative_position() {
    let mut log = new_log();
    // Sea centroid ≈ (25°N, 38.5°E); mountains centroid ≈ (51°N, 27°E): ~26° north, ~11° west.
    let sea = add(
        &mut log,
        "feature.desert",
        vec![("area", band(15.0, 35.0, 30.0, 47.0))],
    );
    let north = add(
        &mut log,
        "feature.mountain_range",
        vec![("spine", common::line(&[(50.0, 20.0), (52.0, 35.0)]))],
    );
    let rel = |log: &mut wb_editlog::EditLog, s, o, r: &str| {
        add(
            log,
            "lore.relative_position",
            vec![
                ("subject", Value::Entity(s)),
                ("object", Value::Entity(o)),
                ("relation", Value::from(r)),
            ],
        )
    };
    let ok = rel(&mut log, north, sea, "north_of");
    let bad = rel(&mut log, sea, north, "north_of");
    let west = rel(&mut log, north, sea, "west_of");
    let east = rel(&mut log, north, sea, "east_of");
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let s = log.state();

    assert_eq!(
        RelativePositionCheck.check(s.entity(ok).unwrap(), &ctx)[0].kind,
        FindingKind::Support
    );
    let f = RelativePositionCheck.check(s.entity(bad).unwrap(), &ctx);
    assert_eq!(f[0].code.as_str(), "lore.relative_position_violated");
    assert_eq!(
        f[0].badness, 1.0,
        "≈ 26° wrong way → 0.2 + 26/20 capped at 1"
    );
    assert_eq!(f[0].related, vec![north]);
    assert_eq!(
        RelativePositionCheck.check(s.entity(west).unwrap(), &ctx)[0].kind,
        FindingKind::Support
    );
    let e = RelativePositionCheck.check(s.entity(east).unwrap(), &ctx);
    assert_eq!(e[0].code.as_str(), "lore.relative_position_violated");
    assert!(
        e[0].badness > 0.6 && e[0].badness < 0.9,
        "≈ 11° west → ≈ 0.75, got {}",
        e[0].badness
    );
}
