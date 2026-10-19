//! Release-only budget (spec §9): a thirteen-body system with twenty moons computes a
//! full year of daily insolation and tides within budget.

mod common;

use common::{add, aethoria, f, i, t};
use wb_astro::SkyModel;
use wb_editlog::{EditLog, EntityId, Value};
use wb_grid::LatLon;

/// Aethoria plus seventeen more moons, spread over the gas giants and the home world.
fn heavy_system() -> EditLog {
    let mut log = aethoria();
    let hosts: Vec<EntityId> = log
        .state()
        .live()
        .filter(|e| {
            e.kind() == Some("astro.body")
                && matches!(e.get("class"), Some(Value::Text(c)) if c == "gas_giant")
        })
        .map(|e| e.id)
        .collect();
    let mut n = 0;
    for host in hosts {
        for k in 1..=5 {
            n += 1;
            add(
                &mut log,
                "astro.body",
                vec![
                    ("name", t(&format!("Moonlet {n}"))),
                    ("order", i(k)),
                    ("class", t("moon")),
                    ("parent", Value::Entity(host)),
                    ("period_days", f(3.0 * k as f64)),
                ],
            );
        }
    }
    for k in 1..=2 {
        n += 1;
        add(
            &mut log,
            "astro.body",
            vec![
                ("name", t(&format!("Moonlet {n}"))),
                ("order", i(3 + k)),
                ("class", t("moon")),
                ("parent", Value::Entity(EntityId::PLANET)),
                ("period_days", f(45.0 + 7.0 * k as f64)),
            ],
        );
    }
    log
}

#[test]
#[cfg_attr(debug_assertions, ignore = "release-only performance guard")]
fn a_year_of_daily_sky_is_within_budget() {
    let log = heavy_system();
    let sky = SkyModel::build(log.state()).unwrap();
    assert_eq!(sky.moons_of_home().count(), 5);
    assert!(sky.bodies().count() >= 33, "{}", sky.bodies().count());

    let lat = LatLon::from_degrees(45.0, 0.0);
    let start = std::time::Instant::now();
    let mut total = 0.0;
    for d in 0..390 {
        let day = d as f64;
        total += sky.insolation(lat, day);
        total += sky.tide_range_m(day);
    }
    let us = start.elapsed().as_micros();
    assert!(total > 0.0);
    // Measured on the development Mac: ~2.3-2.4 ms native (wasm not measured here).
    // Millisecond resolution was ~50% quantization at a 2 ms measurement, so this now
    // times in microseconds, and the budget is tightened accordingly — still a wide
    // margin over the measured figure, but one that would actually catch a regression.
    let budget = if cfg!(target_family = "wasm") {
        150_000
    } else {
        50_000
    };
    assert!(
        us <= budget,
        "a year of daily sky for 33 bodies took {us} us (budget {budget} us)"
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "release-only performance guard")]
fn evaluating_the_whole_system_is_within_budget() {
    let log = heavy_system();
    let start = std::time::Instant::now();
    let report = common::astro_report(&log);
    let us = start.elapsed().as_micros();
    assert!(report.verdicts.len() >= 33);
    // Every astro checker rebuilds the model per entity, which is the cost this measures.
    // Measured on the development Mac: ~2.7-2.9 ms native (wasm not measured here).
    let budget = if cfg!(target_family = "wasm") {
        150_000
    } else {
        50_000
    };
    assert!(
        us <= budget,
        "evaluated {} verdicts in {us} us (budget {budget} us)",
        report.verdicts.len()
    );
}
