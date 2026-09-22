//! Cross-target determinism gate for verdict reports. Update GOLDEN_REPORT in the same
//! commit as any intentional change to checkers, grading, or encoding.

mod common;

use common::{WestIsLand, add, band, line, new_log, poly};
use wb_constraints::{CheckContext, CheckerRegistry, IssueCode, evaluate, keep_anyway};
use wb_editlog::Value;
use wb_world::{Planet, to_hex};

const GOLDEN_REPORT: &str = "1bac7e1475e760d90bc40f16c1e444984de57f4e8234c65a58b28c8094f721ae";

#[test]
fn golden_report() {
    let mut log = new_log();
    let sea = add(
        &mut log,
        "feature.desert",
        vec![("area", band(15.0, 35.0, -47.0, -30.0))],
    );
    let range = add(
        &mut log,
        "feature.mountain_range",
        vec![("spine", line(&[(38.0, -47.0), (40.0, -30.0)]))],
    );
    add(
        &mut log,
        "lore.area",
        vec![
            ("subject", Value::Entity(sea)),
            ("value", Value::Float(600_000.0)),
            ("unit", Value::from("mi2")),
        ],
    );
    add(
        &mut log,
        "lore.relative_position",
        vec![
            ("subject", Value::Entity(range)),
            ("object", Value::Entity(sea)),
            ("relation", Value::from("south_of")),
        ],
    );
    add(
        &mut log,
        "feature.river",
        vec![("path", line(&[(20.0, 10.0), (20.0, -5.0)]))],
    );
    add(
        &mut log,
        "feature.glacier",
        vec![("area", poly(&[(1.0, -20.0), (1.0, -18.0), (3.0, -18.0)]))],
    );
    let forest = add(
        &mut log,
        "feature.forest",
        vec![
            ("area", poly(&[(50.0, -20.0), (50.0, -18.0), (52.0, -18.0)])),
            ("biome", Value::from("tropical")),
        ],
    );
    add(
        &mut log,
        "rule.region",
        vec![
            (
                "area",
                poly(&[(45.0, -25.0), (45.0, -10.0), (55.0, -10.0), (55.0, -25.0)]),
            ),
            ("rule", Value::from("no_rain")),
        ],
    );
    add(
        &mut log,
        "civ.settlement",
        vec![
            ("at", Value::LatLon(common::d(10.0, 20.0))),
            ("name", Value::from("Kaldros")),
        ],
    );
    keep_anyway(
        &mut log,
        forest,
        &[IssueCode::new("climate.tropical_forest_high_latitude")],
        None,
    )
    .unwrap();

    let reg = CheckerRegistry::with_starter_checks();
    let ctx = CheckContext::new(log.state(), Planet::default(), &WestIsLand);
    let got = to_hex(&evaluate(&ctx, &reg).hash);
    assert_eq!(
        got, GOLDEN_REPORT,
        "report changed; if intentional, set GOLDEN_REPORT = \"{got}\""
    );
}
