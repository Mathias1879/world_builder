mod common;

use common::{WestIsLand, add, band, d, line, new_log, poly};
use wb_constraints::{CheckContext, CheckerRegistry, evaluate};
use wb_editlog::{EditLog, EntityId, Value};
use wb_world::Planet;

/// Deterministic globe-spanning placement: a coprime-stride walk in degrees.
///
/// Latitude stays within ±70° and longitude within ±175°, so the ±4° shapes built
/// around these anchors never touch a pole or the antimeridian.
fn place(i: usize, lat_stride: usize, lon_stride: usize) -> (f64, f64) {
    let lat = -70.0 + ((i * lat_stride) % 141) as f64;
    let lon = -175.0 + ((i * lon_stride) % 351) as f64;
    (lat, lon)
}

/// A small 4-vertex box — the shape most drawn features actually have.
fn small_box(lat: f64, lon: f64) -> Value {
    poly(&[
        (lat, lon),
        (lat, lon + 1.0),
        (lat + 1.0, lon + 1.0),
        (lat + 1.0, lon),
    ])
}

/// Builds the 5 000-constraint world: a realistic mix, not 2 500 identical deserts.
fn world() -> EditLog {
    let mut log = new_log();

    // 500 region rules, each an 8°×8° band (~32 vertices) so the winding test is not
    // trivially short. `no_rain` and `permanent_ice` between them forbid every feature
    // kind below, so every rule really scans every candidate.
    for i in 0..500 {
        let (lat, lon) = place(i, 37, 97);
        let rule = if i % 2 == 0 {
            "no_rain"
        } else {
            "permanent_ice"
        };
        add(
            &mut log,
            "rule.region",
            vec![
                ("area", band(lat - 4.0, lat + 4.0, lon - 4.0, lon + 4.0)),
                ("rule", Value::from(rule)),
            ],
        );
    }

    // 1 500 area features spread over the globe: forests, lakes, deserts.
    let mut features: Vec<EntityId> = Vec::with_capacity(1_500);
    for i in 0..1_500 {
        let (lat, lon) = place(i, 23, 53);
        let (kind, extra) = match i % 3 {
            0 => ("feature.forest", vec![("biome", Value::from("boreal"))]),
            1 => ("feature.lake", Vec::new()),
            _ => ("feature.desert", Vec::new()),
        };
        let mut fields = vec![("area", small_box(lat, lon))];
        fields.extend(extra);
        features.push(add(&mut log, kind, fields));
    }

    // 500 rivers with 3-point paths, so `RiverMouthCheck` runs its lake scan 500 times.
    for i in 0..500 {
        let (lat, lon) = place(i, 41, 71);
        add(
            &mut log,
            "feature.river",
            vec![(
                "path",
                line(&[(lat, lon), (lat + 0.5, lon + 0.5), (lat + 1.0, lon + 1.0)]),
            )],
        );
    }

    // 1 000 lore.area claims, each pointing at one of the drawn features.
    for i in 0..1_000 {
        add(
            &mut log,
            "lore.area",
            vec![
                ("subject", Value::Entity(features[i % features.len()])),
                ("value", Value::Float(3_000.0)),
                ("unit", Value::from("km2")),
            ],
        );
    }

    // 1 500 settlements — points, so `CoastCheck` samples every one of them.
    for i in 0..1_500 {
        let (lat, lon) = place(i, 29, 83);
        add(
            &mut log,
            "civ.settlement",
            vec![
                ("at", Value::LatLon(d(lat, lon))),
                ("name", Value::from(format!("Town {i}"))),
            ],
        );
    }

    log
}

#[test]
#[cfg_attr(debug_assertions, ignore = "release-only performance guard")]
fn evaluating_5000_constraints_is_within_budget() {
    let log = world();
    let reg = CheckerRegistry::with_starter_checks();
    // A terrain that answers `Some` everywhere, so `CoastCheck` actually samples
    // instead of bailing out on the first `None`.
    let ctx = CheckContext::new(log.state(), Planet::default(), &WestIsLand);
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
