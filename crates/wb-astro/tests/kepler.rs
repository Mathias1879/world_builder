use std::f64::consts::{PI, TAU};
use wb_astro::{
    AU_M, G, KEPLER_ITERATIONS, SOLAR_MASS_KG, eccentric_anomaly, ecliptic_longitude, mean_anomaly,
    orbit_position, period_s, radius_m, semi_major_m, true_anomaly, wrap_angle,
};

const YEAR_S: f64 = 365.256_363_004 * 86_400.0;

fn close(got: f64, want: f64, rel: f64) {
    let err = (got - want).abs() / want.abs();
    assert!(
        err <= rel,
        "got {got}, want {want} (relative error {err} > {rel})"
    );
}

#[test]
fn one_au_around_the_sun_is_one_year() {
    close(period_s(AU_M, SOLAR_MASS_KG), YEAR_S, 0.001);
}

#[test]
fn one_year_around_the_sun_is_one_au() {
    close(semi_major_m(YEAR_S, SOLAR_MASS_KG), AU_M, 0.001);
}

#[test]
fn period_and_distance_round_trip() {
    for au in [0.39, 0.72, 1.0, 5.2, 30.1, 39.5] {
        let a = au * AU_M;
        let back = semi_major_m(period_s(a, SOLAR_MASS_KG), SOLAR_MASS_KG);
        close(back, a, 1e-12);
    }
}

#[test]
fn the_moon_orbits_earth_in_27_days() {
    let earth = 5.972e24;
    let p = period_s(3.844e8, earth);
    close(p / 86_400.0, 27.32, 0.01);
}

#[test]
fn a_circular_orbit_has_eccentric_anomaly_equal_to_mean() {
    for m in [0.0, 0.5, 1.0, PI, 4.0, 6.0] {
        let diff = (eccentric_anomaly(m, 0.0) - wrap_angle(m)).abs();
        assert!(
            diff < 1e-12,
            "eccentric_anomaly({m}, 0.0) differs from wrap_angle({m}) by {diff}"
        );
    }
    // `close` divides by `want`; assert the exact identity directly too.
    assert_eq!(eccentric_anomaly(0.0, 0.0), 0.0);
}

#[test]
fn the_solver_satisfies_keplers_equation() {
    for &e in &[0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 0.95] {
        for i in 0..32 {
            let m = -PI + TAU * i as f64 / 32.0;
            let ea = eccentric_anomaly(m, e);
            let residual = (ea - e * libm::sin(ea)) - wrap_angle(m);
            assert!(
                residual.abs() < 1e-9,
                "e = {e}, M = {m}: residual {residual} after {KEPLER_ITERATIONS} iterations"
            );
        }
    }
}

#[test]
fn the_solver_is_bounded_for_an_out_of_range_eccentricity() {
    // Validation keeps e ≤ 0.95, but a hostile file could carry anything.
    for &e in &[1.0, 7.5, -3.0, f64::INFINITY] {
        let ea = eccentric_anomaly(1.0, e);
        assert!(ea.is_finite(), "e = {e} gave {ea}");
    }
    assert!(eccentric_anomaly(1.0, f64::NAN).is_finite());
}

#[test]
fn non_finite_inputs_give_finite_output() {
    // radius_m guards against non-finite a_m and ea
    for &a in &[f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let r = radius_m(a, 0.2, PI / 4.0);
        assert!(r.is_finite(), "radius_m({a}, 0.2, π/4) gave {r}");
    }
    for &ea in &[f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let r = radius_m(AU_M, 0.2, ea);
        assert!(r.is_finite(), "radius_m(AU_M, 0.2, {ea}) gave {r}");
    }

    // true_anomaly guards against non-finite ea
    for &ea in &[f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let ta = true_anomaly(ea, 0.3);
        assert!(ta.is_finite(), "true_anomaly({ea}, 0.3) gave {ta}");
    }

    // orbit_position guards against non-finite a_m, inclination_rad, and mean_anomaly
    for &a in &[f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let p = orbit_position(a, 0.2, 0.1, PI / 4.0);
        assert!(p.x.is_finite(), "orbit_position({a}, ...).x is not finite");
        assert!(p.y.is_finite(), "orbit_position({a}, ...).y is not finite");
        assert!(p.z.is_finite(), "orbit_position({a}, ...).z is not finite");
    }
    for &inc in &[f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let p = orbit_position(AU_M, 0.2, inc, PI / 4.0);
        assert!(
            p.x.is_finite(),
            "orbit_position(..., {inc}, ...).x is not finite"
        );
        assert!(
            p.y.is_finite(),
            "orbit_position(..., {inc}, ...).y is not finite"
        );
        assert!(
            p.z.is_finite(),
            "orbit_position(..., {inc}, ...).z is not finite"
        );
    }
    for &m in &[f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let p = orbit_position(AU_M, 0.2, 0.1, m);
        assert!(p.x.is_finite(), "orbit_position(..., {m}).x is not finite");
        assert!(p.y.is_finite(), "orbit_position(..., {m}).y is not finite");
        assert!(p.z.is_finite(), "orbit_position(..., {m}).z is not finite");
    }
}

#[test]
fn periapsis_and_apoapsis_are_where_they_should_be() {
    let a = AU_M;
    let e = 0.2;
    close(radius_m(a, e, 0.0), a * (1.0 - e), 1e-12);
    close(radius_m(a, e, PI), a * (1.0 + e), 1e-12);
    // At mean anomaly 0 the body sits at periapsis on the +x axis.
    let p = orbit_position(a, e, 0.0, 0.0);
    close(p.x, a * (1.0 - e), 1e-9);
    assert!(p.y.abs() < 1.0 && p.z.abs() < 1.0, "{p:?}");
    close(p.length(), a * (1.0 - e), 1e-9);
}

#[test]
fn a_circular_orbit_keeps_a_constant_radius() {
    let a = 2.5 * AU_M;
    for i in 0..16 {
        let m = TAU * i as f64 / 16.0;
        close(orbit_position(a, 0.0, 0.0, m).length(), a, 1e-9);
    }
}

#[test]
fn inclination_tilts_the_orbit_out_of_the_plane() {
    let a = AU_M;
    let inc = 30.0_f64.to_radians();
    // A quarter turn after periapsis the body is at maximum height for its tilt.
    let p = orbit_position(a, 0.0, inc, PI / 2.0);
    close(p.z, a * libm::sin(inc), 1e-9);
    close(p.length(), a, 1e-9);
    // Zero inclination stays flat everywhere.
    for i in 0..8 {
        let flat = orbit_position(a, 0.3, 0.0, TAU * i as f64 / 8.0);
        assert_eq!(flat.z, 0.0);
    }
}

#[test]
fn a_full_period_returns_to_the_start() {
    let a = 1.7 * AU_M;
    let p = period_s(a, SOLAR_MASS_KG);
    let start = orbit_position(a, 0.4, 0.1, mean_anomaly(0.0, p));
    let after = orbit_position(a, 0.4, 0.1, mean_anomaly(p, p));
    close(after.x, start.x, 1e-9);
    assert!((after.y - start.y).abs() < 1.0, "{after:?} vs {start:?}");
}

#[test]
fn mean_anomaly_advances_linearly() {
    let p = 1_000.0;
    close(mean_anomaly(250.0, p), TAU / 4.0, 1e-12);
    close(mean_anomaly(500.0, p), PI, 1e-12);
    assert_eq!(mean_anomaly(0.0, p), 0.0);
    // A zero or negative period cannot divide; it means "does not move".
    assert_eq!(mean_anomaly(5.0, 0.0), 0.0);
}

#[test]
fn true_anomaly_matches_mean_for_a_circle_and_leads_it_before_apoapsis() {
    close(true_anomaly(PI / 2.0, 0.0), PI / 2.0, 1e-12);
    let ta = true_anomaly(PI / 2.0, 0.5);
    assert!(ta > PI / 2.0, "true anomaly {ta} should lead E for e > 0");
}

#[test]
fn longitudes_and_angles_wrap_into_the_half_open_turn() {
    assert_eq!(ecliptic_longitude(wb_grid::Vec3::new(1.0, 0.0, 0.0)), 0.0);
    close(
        ecliptic_longitude(wb_grid::Vec3::new(0.0, 1.0, 0.0)),
        PI / 2.0,
        1e-12,
    );
    assert!(wrap_angle(TAU + 0.25) - 0.25 < 1e-12);
    assert!(wrap_angle(-TAU - 0.25) + 0.25 < 1e-12);
    for a in [-10.0, -PI, 0.0, PI, 10.0, 100.0] {
        let w = wrap_angle(a);
        assert!(w > -PI - 1e-12 && w <= PI + 1e-12, "wrap_angle({a}) = {w}");
    }
}

#[test]
fn g_is_used_consistently() {
    // Kepler's third law in the form the solver uses, checked against the raw constant.
    let a = 3.0 * AU_M;
    let p = period_s(a, SOLAR_MASS_KG);
    let implied = 4.0 * PI * PI * a * a * a / (G * p * p);
    close(implied, SOLAR_MASS_KG, 1e-9);
}
