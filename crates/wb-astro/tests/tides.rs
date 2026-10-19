mod common;

use common::{aethoria, earth_system, named};
use wb_astro::{SkyModel, tidal_amplitude_m};
use wb_editlog::EntityId;

fn close(got: f64, want: f64, rel: f64) {
    let err = (got - want).abs() / want.abs();
    assert!(
        err <= rel,
        "got {got}, want {want} (relative error {err} > {rel})"
    );
}

#[test]
fn the_moons_equilibrium_amplitude_is_about_half_a_metre() {
    let a = tidal_amplitude_m(7.342e22, 3.844e8, 6.371e6, 9.80665);
    close(a, 0.54, 0.05);
    // The Sun's is a bit under half the Moon's — the textbook ratio.
    let s = tidal_amplitude_m(1.98841e30, 1.495978707e11, 6.371e6, 9.80665);
    close(s / a, 0.46, 0.1);
}

#[test]
fn an_impossible_distance_gives_zero_rather_than_infinity() {
    assert_eq!(tidal_amplitude_m(7.3e22, 0.0, 6.4e6, 9.8), 0.0);
    assert_eq!(tidal_amplitude_m(7.3e22, -1.0, 6.4e6, 9.8), 0.0);
    assert_eq!(tidal_amplitude_m(7.3e22, 3.8e8, 6.4e6, 0.0), 0.0);
    assert!(tidal_amplitude_m(f64::NAN, 3.8e8, 6.4e6, 9.8).is_finite());
}

#[test]
fn earths_spring_tides_are_about_twice_its_neaps() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let ranges: Vec<f64> = (0..60).map(|d| sky.tide_range_m(d as f64)).collect();
    let spring = ranges.iter().copied().fold(0.0_f64, f64::max);
    let neap = ranges.iter().copied().fold(f64::MAX, f64::min);
    // Equilibrium theory, not a tide table: ~1.5 m springs, ~0.6 m neaps, so the ratio
    // sits near 2.6 where the observed ratio is about 2.2 (real oceans resonate).
    assert!((1.0..2.5).contains(&spring), "spring range = {spring}");
    assert!((0.2..1.0).contains(&neap), "neap range = {neap}");
    let ratio = spring / neap;
    assert!((2.0..3.2).contains(&ratio), "spring/neap = {ratio}");
}

#[test]
fn three_moons_beat_into_a_repeatable_maximum() {
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
    assert!(height > 0.0 && height.is_finite(), "peak height {height}");

    // The three moons (10, 15 and 30-day cycles) realign every 30 days, but the primary
    // star's tide is not commensurate with that beat: its amplitude is 0.170 m against
    // the outermost moon's 0.169 m, and its doubled phase rotates 55.4 deg per 30 days,
    // so the combined peak wanders through the year. It realigns exactly once a year,
    // because 390 local days is both one year and 13 x 30 days.
    for years in [1, 10] {
        let (_, d) = peak_in(years * 390);
        assert_eq!(
            d - years * 390,
            day,
            "the spring tide should recur on the same day of each year, not day {d}"
        );
    }

    // And the beat is real: the peak towers over the quietest day of the same cycle.
    let trough = (0..30)
        .map(|d| sky.tide_range_m(d as f64))
        .fold(f64::MAX, f64::min);
    assert!(
        height > trough * 1.5,
        "three moons should beat hard: {height} against {trough}"
    );
}

#[test]
fn tides_are_finite_and_non_negative_everywhere() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    for d in [-100.0, 0.0, 0.5, 123.4, 390.0, 10_000.0] {
        let r = sky.tide_range_m(d);
        assert!(r.is_finite() && r >= 0.0, "tide_range_m({d}) = {r}");
    }
}

#[test]
fn a_world_with_no_moons_still_has_solar_tides() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let with_moon = sky.tide_range_m(0.0);
    let mut log = earth_system();
    let luna = named(&log, "Luna");
    let mut tx = log.transact("delete");
    tx.delete(luna);
    tx.commit().unwrap();
    let solar_only = SkyModel::build(log.state()).unwrap().tide_range_m(0.0);
    assert!(solar_only > 0.0, "the star alone raises a tide");
    assert!(
        solar_only < with_moon,
        "{solar_only} should be under {with_moon}"
    );
}

#[test]
fn a_moon_runs_through_its_whole_phase_cycle() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let luna = sky.moons_of_home().next().unwrap().id;
    let phases: Vec<f64> = (0..28)
        .map(|d| sky.phase(luna, d as f64).unwrap())
        .collect();
    for p in &phases {
        assert!((0.0..=1.0).contains(p), "phase {p} outside 0..1");
    }
    let full = phases.iter().copied().fold(0.0_f64, f64::max);
    let new = phases.iter().copied().fold(1.0_f64, f64::min);
    assert!(full > 0.97, "a lunar month should reach full: {full}");
    assert!(new < 0.03, "a lunar month should reach new: {new}");
}

#[test]
fn stars_are_always_full_and_unknown_bodies_have_no_phase() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    for s in sky.stars() {
        assert_eq!(sky.phase(s.id, 17.0), Some(1.0));
    }
    assert_eq!(
        sky.phase(EntityId::PLANET, 3.0),
        None,
        "the home world sees itself"
    );
    let ghost = EntityId {
        op: wb_editlog::OpId {
            lamport: 77,
            actor: wb_editlog::ActorId([2; 16]),
        },
        n: 1,
    };
    assert_eq!(sky.phase(ghost, 0.0), None);
    assert_eq!(sky.apparent_brightness(ghost, 0.0), None);
}

#[test]
fn the_primary_star_is_the_brightest_thing_in_the_sky() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let sun = sky
        .apparent_brightness(sky.primary_star().id, 0.0)
        .expect("the sun is visible");
    // Magnitude-like: smaller is brighter, and the primary is near −26 as from Earth.
    assert!(sun < -20.0, "the primary's magnitude = {sun}");
    for b in sky.bodies() {
        if b.is_primary_star || b.is_home {
            continue;
        }
        if let Some(m) = sky.apparent_brightness(b.id, 0.0) {
            assert!(m > sun, "{} at {m} outshines the primary at {sun}", b.name);
        }
    }
}

#[test]
fn luminar_outshines_the_outer_worlds() {
    // The lore calls Luminar the bright morning star; being near and reflective, it should
    // beat the distant giants from this engine's numbers alone.
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let brightest_day = |name: &str| {
        let id = named(&log, name);
        (0..390)
            .filter_map(|d| sky.apparent_brightness(id, d as f64))
            .fold(f64::MAX, f64::min)
    };
    let luminar = brightest_day("Luminar");
    for far in ["Frostweaver", "Rimefrost", "Voidwalker", "Shadowkeep"] {
        assert!(
            luminar < brightest_day(far),
            "Luminar at {luminar} should outshine {far} at {}",
            brightest_day(far)
        );
    }
}

#[test]
fn brightness_dims_with_phase() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let luna = sky.moons_of_home().next().unwrap().id;
    let mut brightest = (f64::MAX, 0);
    let mut dimmest = (f64::MIN, 0);
    for d in 0..28 {
        if let Some(m) = sky.apparent_brightness(luna, d as f64) {
            if m < brightest.0 {
                brightest = (m, d);
            }
            if m > dimmest.0 {
                dimmest = (m, d);
            }
        }
    }
    assert!(
        dimmest.0 - brightest.0 > 1.0,
        "a full moon should be more than a magnitude brighter than a thin one: {brightest:?} / {dimmest:?}"
    );
    let full_phase = sky.phase(luna, brightest.1 as f64).unwrap();
    assert!(
        full_phase > 0.8,
        "the brightest night should be near full: {full_phase}"
    );
}
