//! Spec §9: real planetary distances must reproduce real periods, and Neptune and Pluto
//! must come out protected by their resonance rather than crossing.

mod common;

use common::{
    add, add_gregorian_calendar, astro_report, codes, f, findings, i, named, new_log, set, t,
};
use wb_astro::SkyModel;
use wb_editlog::{EditLog, EntityId};

/// name, order, class, semi-major axis in AU, eccentricity, real period in years.
const PLANETS: [(&str, i64, &str, f64, f64, f64); 8] = [
    ("Mercury", 1, "rocky", 0.387_098, 0.2056, 0.240_846),
    ("Venus", 2, "rocky", 0.723_332, 0.0068, 0.615_198),
    ("Mars", 4, "rocky", 1.523_68, 0.0934, 1.880_85),
    ("Jupiter", 5, "gas_giant", 5.204_4, 0.0489, 11.862),
    ("Saturn", 6, "gas_giant", 9.582_6, 0.0565, 29.457),
    ("Uranus", 7, "ice_giant", 19.218_4, 0.0457, 84.021),
    ("Neptune", 8, "ice_giant", 30.07, 0.0086, 164.8),
    ("Pluto", 9, "rocky", 39.482, 0.2488, 247.94),
];

fn solar_system() -> EditLog {
    let mut log = new_log();
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Sol")),
            ("role", t("primary")),
            ("mass_solar", f(1.0)),
            ("luminosity_solar", f(1.0)),
        ],
    );
    set(
        &mut log,
        EntityId::PLANET,
        vec![
            ("name", t("Earth")),
            ("order", i(3)),
            ("year_days", f(365.2422)),
            ("day_hours", f(24.0)),
            ("axial_tilt_deg", f(23.44)),
            ("radius_km", f(6_371.0)),
            ("gravity_ms2", f(9.80665)),
        ],
    );
    for (name, order, class, au, e, _) in PLANETS {
        add(
            &mut log,
            "astro.body",
            vec![
                ("name", t(name)),
                ("order", i(order)),
                ("class", t(class)),
                ("distance_au", f(au)),
                ("eccentricity", f(e)),
            ],
        );
    }
    add_gregorian_calendar(&mut log);
    log
}

#[test]
fn real_distances_reproduce_real_periods_within_one_percent() {
    let log = solar_system();
    let sky = SkyModel::build(log.state()).unwrap();
    let year_s = 365.2422 * 24.0 * 3_600.0;
    for (name, _, _, _, _, period_years) in PLANETS {
        let got = sky.body(named(&log, name)).unwrap().period_s / year_s;
        let err = (got - period_years).abs() / period_years;
        assert!(
            err <= 0.01,
            "{name}: derived {got} years against the real {period_years} ({err} off)"
        );
    }
    // And the home world's own orbit, driven the other way round, is 1 AU.
    let err = (sky.home().a_m / wb_astro::AU_M - 1.0).abs();
    assert!(
        err <= 0.001,
        "Earth came out at {} AU",
        sky.home().a_m / wb_astro::AU_M
    );
}

#[test]
fn neptune_and_pluto_are_a_resonance_not_a_crossing() {
    let log = solar_system();
    let r = astro_report(&log);
    assert!(
        findings(&r, "astro.orbit_crossing").is_empty(),
        "nothing in the real solar system crosses unprotected: {:?}",
        codes(&r)
    );
    let protects = findings(&r, "astro.resonance_protects");
    assert_eq!(protects.len(), 1, "{:?}", codes(&r));
    let (neptune, pluto) = (named(&log, "Neptune"), named(&log, "Pluto"));
    assert!(
        protects[0].related == vec![pluto] || protects[0].related == vec![neptune],
        "the finding should name the other body of the pair"
    );
}

#[test]
fn the_real_solar_system_raises_nothing_else() {
    let r = astro_report(&solar_system());
    let issues: Vec<&str> = r
        .verdicts
        .iter()
        .flat_map(|v| v.issues.iter())
        .map(|f| f.code.as_str())
        .collect();
    assert!(
        issues.is_empty(),
        "the real solar system should be entirely plausible, got {issues:?}"
    );
    assert_eq!(r.counts.implausible, 0);
    assert_eq!(r.counts.stretch, 0);
}
