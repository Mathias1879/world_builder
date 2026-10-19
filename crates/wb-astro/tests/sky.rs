mod common;

use common::{add, aethoria, earth_system, f, i, named, set, t};
use std::f64::consts::PI;
use wb_astro::{SkyModel, daily_mean_factor, hour_angle};
use wb_editlog::{EntityId, Value};
use wb_grid::LatLon;

fn close(got: f64, want: f64, rel: f64) {
    let err = (got - want).abs() / want.abs();
    assert!(
        err <= rel,
        "got {got}, want {want} (relative error {err} > {rel})"
    );
}

/// Annual mean of `f` over a year, by whole days.
fn annual_mean(days: f64, f: impl Fn(f64) -> f64) -> f64 {
    let n = days.round() as i64;
    let mut sum = 0.0;
    for d in 0..n {
        sum += f(d as f64);
    }
    sum / n as f64
}

#[test]
fn the_hour_angle_covers_equinox_polar_day_and_polar_night() {
    // Equinox anywhere: half a turn of daylight.
    close(hour_angle(0.5, 0.0), PI / 2.0, 1e-12);
    close(hour_angle(0.0, 0.0), PI / 2.0, 1e-12);
    // Northern summer at the north pole: all day.
    close(hour_angle(PI / 2.0, 0.4), PI, 1e-9);
    // And no day at all at the south pole.
    assert_eq!(hour_angle(-PI / 2.0, 0.4), 0.0);
    for (lat, decl) in [(0.0, 0.0), (1.0, 0.3), (-1.2, -0.2), (PI / 2.0, 0.0)] {
        let h = hour_angle(lat, decl);
        assert!((0.0..=PI).contains(&h), "hour_angle({lat}, {decl}) = {h}");
    }
}

#[test]
fn the_daily_mean_factor_is_never_negative() {
    for i in 0..40 {
        let lat = -PI / 2.0 + PI * i as f64 / 39.0;
        for j in 0..20 {
            let decl = -0.41 + 0.82 * j as f64 / 19.0;
            let k = daily_mean_factor(lat, decl);
            assert!(
                (0.0..=1.0).contains(&k) && k.is_finite(),
                "k({lat}, {decl}) = {k}"
            );
        }
    }
    // At the sub-stellar point of an equinox equator, a quarter of the sub-stellar flux.
    close(daily_mean_factor(0.0, 0.0), 1.0 / PI, 1e-12);
}

#[test]
fn earths_annual_mean_insolation_matches_the_published_figures() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let equator = annual_mean(365.2422, |d| {
        sky.insolation(LatLon::from_degrees(0.0, 0.0), d)
    });
    let pole = annual_mean(365.2422, |d| {
        sky.insolation(LatLon::from_degrees(90.0, 0.0), d)
    });
    // Top-of-atmosphere annual means: ~417 W/m² at the equator, ~173 at the pole.
    close(equator, 417.0, 0.02);
    close(pole, 173.0, 0.02);
    // The global pattern: the equator gets well over twice the pole.
    assert!(equator > 2.0 * pole);
}

#[test]
fn insolation_follows_the_seasons() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let north = LatLon::from_degrees(60.0, 0.0);
    let south = LatLon::from_degrees(-60.0, 0.0);
    // Day 0 is periapsis with the reference direction at ecliptic longitude 0, so the
    // declination is zero: an equinox. A quarter year on, one hemisphere is in summer.
    let quarter = 365.2422 / 4.0;
    let (n0, s0) = (sky.insolation(north, 0.0), sky.insolation(south, 0.0));
    close(n0, s0, 0.02);

    let primary = sky.primary_star().id;
    // Day 0 puts the home world at periapsis on +x, so the star is seen at ecliptic
    // longitude 180° and the declination is zero: an equinox. A quarter year on it is
    // seen at 270°, so the declination is -ε and the southern hemisphere is in summer.
    // This pins declination against the orbit's phase; the assertions below pin
    // insolation's response to declination. Together they are asymmetric, so a globally
    // negated declination fails here — which the previous symmetric form did not.
    let decl_quarter = sky.declination(sky.position(primary, quarter).expect("the primary"));
    assert!(
        decl_quarter < -0.3,
        "a quarter year in, the declination should be about -23.44 deg, got {} deg",
        decl_quarter.to_degrees()
    );

    let (n1, s1) = (
        sky.insolation(north, quarter),
        sky.insolation(south, quarter),
    );
    assert!(
        s1 > n1 * 2.0,
        "with a negative declination the south should be far brighter: north {n1}, south {s1}"
    );
    // Half a year after that solstice, the lopsidedness reverses.
    let n3 = sky.insolation(north, 3.0 * quarter);
    let s3 = sky.insolation(south, 3.0 * quarter);
    assert!(
        (n1 - s1).signum() != (n3 - s3).signum(),
        "seasons must reverse"
    );
}

#[test]
fn insolation_is_hemisphere_symmetric_without_a_tilt() {
    let mut log = earth_system();
    set(
        &mut log,
        EntityId::PLANET,
        vec![("axial_tilt_deg", f(0.0)), ("eccentricity", f(0.0))],
    );
    let sky = SkyModel::build(log.state()).unwrap();
    for d in [0.0, 37.0, 180.5, 300.0] {
        for lat in [5.0, 23.0, 47.0, 80.0] {
            let n = sky.insolation(LatLon::from_degrees(lat, 0.0), d);
            let s = sky.insolation(LatLon::from_degrees(-lat, 40.0), d);
            assert!(
                (n - s).abs() < 1e-9,
                "lat ±{lat} on day {d}: {n} vs {s} — longitude must not matter either"
            );
        }
    }
}

#[test]
fn daylight_is_twelve_hours_at_an_equinox_and_polar_at_a_solstice() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    close(
        sky.daylight_hours(LatLon::from_degrees(0.0, 0.0), 0.0),
        12.0,
        1e-6,
    );
    close(
        sky.daylight_hours(LatLon::from_degrees(51.5, 0.0), 0.0),
        12.0,
        1e-6,
    );
    let quarter = 365.2422 / 4.0;
    let summer = sky.daylight_hours(LatLon::from_degrees(80.0, 0.0), quarter);
    let winter = sky.daylight_hours(LatLon::from_degrees(-80.0, 0.0), quarter);
    assert!(
        (summer == 24.0 && winter == 0.0) || (summer == 0.0 && winter == 24.0),
        "one pole should be in permanent day and the other in permanent night: {summer} / {winter}"
    );
    // London's longest day is between 16 and 17 hours.
    let longest = (0..366)
        .map(|d| sky.daylight_hours(LatLon::from_degrees(51.5, 0.0), d as f64))
        .fold(0.0_f64, f64::max);
    assert!(
        (16.0..17.0).contains(&longest),
        "longest London day = {longest}"
    );
}

#[test]
fn a_thirty_hour_day_is_measured_in_local_hours() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    close(
        sky.daylight_hours(LatLon::from_degrees(0.0, 0.0), 0.0),
        15.0,
        1e-6,
    );
    // The lore: 18 hours of daylight at peak summer, 12 at winter — so the mid-latitude
    // swing has to reach roughly that spread somewhere in the year.
    let lat = LatLon::from_degrees(40.0, 0.0);
    let hours: Vec<f64> = (0..390)
        .map(|d| sky.daylight_hours(lat, d as f64))
        .collect();
    let longest = hours.iter().copied().fold(0.0_f64, f64::max);
    let shortest = hours.iter().copied().fold(30.0_f64, f64::min);
    assert!(longest > 17.0 && longest < 20.0, "longest = {longest}");
    assert!(shortest > 10.0 && shortest < 13.0, "shortest = {shortest}");
}

#[test]
fn a_second_sun_adds_its_own_light() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let lat = LatLon::from_degrees(0.0, 0.0);
    let with_both: Vec<f64> = (0..390).map(|d| sky.insolation(lat, d as f64)).collect();

    // The same system with the companion deleted.
    let mut alone = aethoria();
    let secundus = named(&alone, "Sol Secundus");
    let mut tx = alone.transact("delete");
    tx.delete(secundus);
    tx.commit().unwrap();
    let one = SkyModel::build(alone.state()).unwrap();
    let with_one: Vec<f64> = (0..390).map(|d| one.insolation(lat, d as f64)).collect();

    for (a, b) in with_both.iter().zip(&with_one) {
        assert!(a >= b, "two suns can never be dimmer than one: {a} < {b}");
    }
    let extra: f64 = with_both.iter().sum::<f64>() - with_one.iter().sum::<f64>();
    assert!(extra > 0.0, "the companion contributed nothing");
}

#[test]
fn insolation_is_finite_and_non_negative_everywhere() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    for i in 0..37 {
        let lat = LatLon::from_degrees(-90.0 + 5.0 * i as f64, 17.0);
        for d in [0.0, 1.5, 99.9, 389.0, 390.0, 1_000.0, -5.0] {
            let w = sky.insolation(lat, d);
            assert!(w.is_finite() && w >= 0.0, "insolation({lat:?}, {d}) = {w}");
            let h = sky.daylight_hours(lat, d);
            assert!(h.is_finite() && (0.0..=30.0).contains(&h), "daylight = {h}");
        }
    }
}

#[test]
fn a_starless_sky_is_dark_rather_than_a_panic() {
    // A primary star with zero luminosity: physically silly, perfectly readable.
    let mut log = common::new_log();
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Ember")),
            ("role", t("primary")),
            ("mass_solar", f(1.0)),
            ("luminosity_solar", f(1e-300)),
        ],
    );
    common::add_aethorian_calendar(&mut log);
    let sky = SkyModel::build(log.state()).unwrap();
    let w = sky.insolation(LatLon::from_degrees(10.0, 0.0), 5.0);
    assert!(w.is_finite() && w >= 0.0, "{w}");
    assert!(
        sky.daylight_hours(LatLon::from_degrees(10.0, 0.0), 5.0)
            .is_finite()
    );
    assert!(i(1) == Value::Int(1));
}
