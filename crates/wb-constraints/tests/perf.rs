mod common;

use common::{add, new_log, poly};
use wb_constraints::{CheckContext, CheckerRegistry, UnknownTerrain, evaluate};
use wb_editlog::Value;
use wb_world::Planet;

#[test]
#[cfg_attr(debug_assertions, ignore = "release-only performance guard")]
fn evaluating_5000_constraints_is_within_budget() {
    let mut log = new_log();
    for i in 0..2_500 {
        let lat = -60.0 + (i % 120) as f64;
        let lon = -170.0 + (i / 120) as f64 * 15.0;
        let d = add(
            &mut log,
            "feature.desert",
            vec![(
                "area",
                poly(&[(lat, lon), (lat, lon + 1.0), (lat + 0.5, lon + 1.0)]),
            )],
        );
        add(
            &mut log,
            "lore.area",
            vec![
                ("subject", Value::Entity(d)),
                ("value", Value::Float(3_000.0)),
                ("unit", Value::from("km2")),
            ],
        );
    }
    let reg = CheckerRegistry::with_starter_checks();
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let start = std::time::Instant::now();
    let report = evaluate(&ctx, &reg);
    let ms = start.elapsed().as_millis();
    assert_eq!(report.verdicts.len(), 5_000);
    let budget = if cfg!(target_family = "wasm") {
        600
    } else {
        200
    };
    assert!(
        ms <= budget,
        "evaluated 5000 constraints in {ms} ms (budget {budget} ms)"
    );
}
