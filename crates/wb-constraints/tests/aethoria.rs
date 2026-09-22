mod common;

use common::{add, band, line, new_log};
use wb_constraints::{
    CheckContext, CheckerRegistry, Grade, IssueCode, Preset, Status, UnknownTerrain,
    apply_suggestion, evaluate, keep_anyway, set_realism,
};
use wb_editlog::{EditLog, EntityId, Value};
use wb_world::Planet;

fn verdict_grade(log: &EditLog, id: EntityId) -> (Grade, Status) {
    let reg = CheckerRegistry::with_starter_checks();
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let r = evaluate(&ctx, &reg);
    let v = r
        .verdicts
        .iter()
        .find(|v| v.constraint == id)
        .expect("verdict present");
    (v.grade, v.status)
}

#[test]
fn great_sand_sea_and_thulean_mountains() {
    let mut log = new_log();
    let sea = add(
        &mut log,
        "feature.desert",
        vec![
            ("area", band(15.0, 35.0, 30.0, 47.0)),
            ("name", Value::from("Great Sand Sea")),
        ],
    );
    let thulean = add(
        &mut log,
        "feature.mountain_range",
        vec![
            ("spine", line(&[(38.0, 30.0), (40.0, 47.0)])),
            ("name", Value::from("Thulean Mountains")),
        ],
    );
    let area = add(
        &mut log,
        "lore.area",
        vec![
            ("subject", Value::Entity(sea)),
            ("value", Value::Float(600_000.0)),
            ("unit", Value::from("mi2")),
        ],
    );
    let north = add(
        &mut log,
        "lore.relative_position",
        vec![
            ("subject", Value::Entity(thulean)),
            ("object", Value::Entity(sea)),
            ("relation", Value::from("north_of")),
        ],
    );

    set_realism(&mut log, Preset::EarthStrict.value()).unwrap();
    assert_eq!(verdict_grade(&log, area).0, Grade::Implausible);
    set_realism(&mut log, Preset::PlausibleFantasy.value()).unwrap();
    assert_eq!(verdict_grade(&log, area).0, Grade::Stretch);
    assert_eq!(verdict_grade(&log, north).0, Grade::Plausible);

    // Apply the suggestion: lore now matches the map.
    let reg = CheckerRegistry::with_starter_checks();
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let report = evaluate(&ctx, &reg);
    let s = report
        .verdicts
        .iter()
        .find(|v| v.constraint == area)
        .unwrap()
        .issues[0]
        .suggestions[0]
        .clone();
    apply_suggestion(&mut log, &s).unwrap();
    assert_eq!(verdict_grade(&log, area).0, Grade::Plausible);
    log.undo().unwrap();
    assert_eq!(verdict_grade(&log, area).0, Grade::Stretch);

    // Keep it anyway instead.
    keep_anyway(
        &mut log,
        area,
        &[IssueCode::new("lore.area_mismatch")],
        Some("Cartographers exaggerate"),
    )
    .unwrap();
    assert_eq!(
        verdict_grade(&log, area),
        (Grade::Plausible, Status::Intentional)
    );

    // Swap the mountains south of the sea.
    let mut tx = log.transact("move mountains");
    tx.set(thulean, "spine", line(&[(5.0, 30.0), (7.0, 47.0)]));
    tx.commit().unwrap();
    assert_eq!(verdict_grade(&log, north).0, Grade::Implausible);
}
