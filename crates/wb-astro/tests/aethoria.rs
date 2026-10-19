//! Spec §9 acceptance: the reference world, judged end to end.

mod common;

use common::{aethoria, astro_report, codes, findings, named};
use std::collections::BTreeSet;
use wb_astro::{Date, SkyModel};
use wb_constraints::{IssueCode, keep_anyway};
use wb_grid::LatLon;

#[test]
fn the_system_reads_as_the_lore_describes_it() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    assert_eq!(sky.stars().count(), 2, "two suns");
    assert_eq!(sky.moons_of_home().count(), 3, "three moons");
    assert_eq!(sky.year_days(), 390.0);
    assert_eq!(sky.day_hours(), 30.0);
    assert_eq!(sky.calendar().months.len(), 13);
    assert_eq!(sky.calendar().month_days(), 390);
    assert_eq!(sky.calendar().weekdays.len(), 7, "the Septem");
    assert_eq!(sky.home().order, 5);
    // The lore's thirteen numbered bodies are the ones orbiting the primary: the twelve
    // astro.body entries (The Veil included — it is body 7) plus the home world. Moons
    // orbit their planets, so they are not numbered.
    let numbered = sky
        .bodies()
        .filter(|b| !b.is_star() && b.parent == Some(sky.primary_star().id))
        .count();
    assert_eq!(numbered, 13);
}

#[test]
fn the_three_moons_beat_into_a_repeatable_spring_tide() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    let peak_in = |from: i64| {
        (from..from + 30)
            .map(|d| (sky.tide_range_m(d as f64), d))
            .fold(
                (f64::MIN, from),
                |best, x| if x.0 > best.0 { x } else { best },
            )
    };
    let (height, day) = peak_in(0);
    assert!(height > 0.0 && height.is_finite());
    // The three moons (10, 15 and 30-day cycles) realign every 30 days, but the primary
    // star's tide is not commensurate with that beat — its amplitude rivals the
    // outermost moon's and its doubled phase rotates ~55 deg per 30 days — so the
    // combined peak wanders through the year and realigns exactly once a year, 390 local
    // days being both one year and 13 x 30.
    for years in [1, 10] {
        let (_, d) = peak_in(years * 390);
        assert_eq!(
            d - years * 390,
            day,
            "the spring tide should recur on the same day of each year, not day {d}"
        );
    }
    let trough = (0..30)
        .map(|d| sky.tide_range_m(d as f64))
        .fold(f64::MAX, f64::min);
    assert!(
        height > trough * 1.5,
        "three moons should beat hard: {height} against {trough}"
    );
}

#[test]
fn the_calendar_round_trips_across_centuries() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    for d in [0, 1, 389, 390, 10_000, 390_000, -1, -390_001] {
        assert_eq!(sky.day_index_of(sky.date_of(d)), Some(d));
    }
    // Mehr 1 of year 1 is a Solday, and the year is 13 months of 30.
    assert_eq!(
        sky.date_of(0),
        Date {
            year: 1,
            month_index: 1,
            day: 1,
            weekday_index: 0
        }
    );
    assert_eq!(sky.calendar().weekdays[0], "Solday");
    assert_eq!(sky.date_of(360).month_index, 13);
}

#[test]
fn sol_secundus_is_judged_exactly_as_the_spec_says() {
    let log = aethoria();
    let r = astro_report(&log);
    let secundus = named(&log, "Sol Secundus");

    // It destabilizes the outer system, naming the bodies at risk with real numbers.
    let destab = findings(&r, "astro.companion_destabilizes");
    assert_eq!(destab.len(), 1, "{:?}", codes(&r));
    assert!(!destab[0].related.is_empty());
    assert!(destab[0].related.contains(&named(&log, "Goliath")));
    assert!(
        !destab[0].related.contains(&wb_editlog::EntityId::PLANET),
        "Aethoria itself is inside the critical distance and survives"
    );

    // And "it returns every year" contradicts a 6 AU orbit.
    let mismatch = findings(&r, "astro.companion_period_mismatch");
    assert_eq!(mismatch.len(), 1, "{:?}", codes(&r));
    for key in [
        "stated_period_years",
        "implied_period_years",
        "stated_distance_au",
        "implied_distance_au",
    ] {
        assert!(mismatch[0].params.contains_key(key), "missing {key}");
    }
    // Both findings hang off the companion's own verdict.
    let verdict = r
        .verdicts
        .iter()
        .find(|v| v.constraint == secundus)
        .expect("the companion gets a verdict");
    assert_eq!(verdict.issues.len(), 2);
    assert!(
        verdict.score < 0.2,
        "two heavy issues: score {}",
        verdict.score
    );
}

#[test]
fn the_resonance_checker_names_a_pair() {
    let log = aethoria();
    let r = astro_report(&log);
    let resonances = findings(&r, "astro.resonance");
    assert!(
        !resonances.is_empty(),
        "no resonance found: {:?}",
        codes(&r)
    );
    // Goliath and Stormheart are a 2:1, as the lore's "orbital resonance" implies. The
    // finding is raised from Goliath, the lower id, so the ratio is stated as its own
    // period over Stormheart's: 1:2.
    let pair = resonances
        .iter()
        .find(|f| f.related.contains(&named(&log, "Stormheart")))
        .expect("Goliath and Stormheart");
    assert!(matches!(
        pair.params.get("p"),
        Some(wb_constraints::Param::Int(1))
    ));
    assert!(matches!(
        pair.params.get("q"),
        Some(wb_constraints::Param::Int(2))
    ));
}

#[test]
fn the_worlds_verdicts_are_exactly_these() {
    // The whole point of the engine: a world-builder can see every judgement at once.
    let log = aethoria();
    let r = astro_report(&log);
    let issues: BTreeSet<&str> = r
        .verdicts
        .iter()
        .flat_map(|v| v.issues.iter())
        .map(|f| f.code.as_str())
        .collect();
    let expected: BTreeSet<&str> = [
        // Sol Secundus, as above.
        "astro.companion_destabilizes",
        "astro.companion_period_mismatch",
        // Goliath shares its number with Sol Secundus, so one of them is out of sequence.
        "astro.order_mismatch",
        // Two crossings, both from the lore: Voidwalker's highly elliptical orbit reaches
        // inside Rimefrost's, and Goliath "shares orbit with Sol Secundus".
        "astro.orbit_crossing",
    ]
    .into_iter()
    .collect();
    assert_eq!(issues, expected, "{:?}", codes(&r));
    assert!(findings(&r, "astro.calendar_sum_mismatch").is_empty());
    assert!(findings(&r, "astro.outside_habitable_zone").is_empty());
    assert!(findings(&r, "astro.tidal_locking").is_empty());
}

#[test]
fn keeping_the_lore_anyway_marks_it_intentional() {
    let mut log = aethoria();
    let secundus = named(&log, "Sol Secundus");
    keep_anyway(
        &mut log,
        secundus,
        &[
            IssueCode::new("astro.companion_destabilizes"),
            IssueCode::new("astro.companion_period_mismatch"),
        ],
        Some("Sol Secundus is held in place by the Weave"),
    )
    .unwrap();
    let r = astro_report(&log);
    let verdict = r
        .verdicts
        .iter()
        .find(|v| v.constraint == secundus)
        .expect("a verdict");
    assert!(verdict.issues.is_empty());
    assert_eq!(verdict.intentional.len(), 2);
    assert_eq!(verdict.status, wb_constraints::Status::Intentional);
    assert_eq!(verdict.score, 1.0);
}

#[test]
fn the_sky_is_legible_all_year() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let lat = LatLon::from_degrees(35.0, 0.0);
    for d in 0..390 {
        let day = d as f64;
        assert!(sky.insolation(lat, day) > 0.0);
        let hours = sky.daylight_hours(lat, day);
        assert!((0.0..=30.0).contains(&hours));
        assert!(sky.tide_range_m(day) >= 0.0);
        for b in sky.bodies() {
            if b.is_home {
                continue;
            }
            assert!(sky.position(b.id, day).is_some());
            if let Some(p) = sky.phase(b.id, day) {
                assert!((0.0..=1.0).contains(&p));
            }
        }
    }
    // Dual Light: the two suns are both up for part of the year, so the companion's
    // contribution peaks well above its annual mean.
    let companion = named(&log, "Sol Secundus");
    let distances: Vec<f64> = (0..390)
        .map(|d| sky.position(companion, d as f64).unwrap().length())
        .collect();
    let nearest = distances.iter().copied().fold(f64::MAX, f64::min);
    let farthest = distances.iter().copied().fold(0.0_f64, f64::max);
    assert!(
        farthest > nearest * 1.3,
        "the companion should approach and recede: {nearest} to {farthest}"
    );
}
