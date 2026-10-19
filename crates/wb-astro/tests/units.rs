use wb_astro::{
    AU_M, BodyClass, EARTH_GRAVITY_MS2, EARTH_RADIUS_KM, G, SOLAR_MASS_KG,
    luminosity_from_mass_solar, mass_from_density, mass_from_gravity,
};

fn close(got: f64, want: f64, rel: f64) {
    let err = (got - want).abs() / want.abs();
    assert!(
        err <= rel,
        "got {got}, want {want} (relative error {err} > {rel})"
    );
}

#[test]
fn class_strings_round_trip() {
    for s in ["rocky", "water", "gas_giant", "ice_giant", "belt", "moon"] {
        let c = BodyClass::parse(s).expect("spec class");
        assert_eq!(c.as_str(), s);
    }
    assert_eq!(BodyClass::parse("planet"), None);
    assert_eq!(BodyClass::parse("star"), None);
    assert_eq!(BodyClass::parse(""), None);
}

#[test]
fn class_defaults_match_the_spec_table() {
    let table = [
        ("rocky", 5_000.0, 5_000.0, 0.15),
        ("water", 6_500.0, 3_000.0, 0.30),
        ("gas_giant", 60_000.0, 1_300.0, 0.50),
        ("ice_giant", 25_000.0, 1_600.0, 0.50),
        ("moon", 1_500.0, 3_300.0, 0.12),
    ];
    for (name, radius_km, density, albedo) in table {
        let d = BodyClass::parse(name).expect("spec class").defaults();
        assert_eq!(d.radius_km, radius_km, "{name} radius");
        assert_eq!(d.density_kgm3, density, "{name} density");
        assert_eq!(d.albedo, albedo, "{name} albedo");
    }
    // A belt has no body radius or density; only its albedo is meaningful.
    assert_eq!(BodyClass::Belt.defaults().albedo, 0.10);
}

#[test]
fn earth_gravity_and_radius_give_earths_mass() {
    let m = mass_from_gravity(EARTH_GRAVITY_MS2, EARTH_RADIUS_KM * 1_000.0);
    close(m, 5.972e24, 0.01);
}

#[test]
fn density_and_radius_give_earths_mass() {
    let m = mass_from_density(EARTH_RADIUS_KM * 1_000.0, 5_514.0);
    close(m, 5.972e24, 0.01);
}

#[test]
fn the_sun_is_one_solar_mass_and_one_solar_luminosity() {
    close(luminosity_from_mass_solar(1.0), 1.0, 1e-12);
    // A third of a solar mass is far dimmer than a third of the luminosity.
    let l = luminosity_from_mass_solar(1.0 / 3.0);
    assert!(l < 0.03, "L(M/3) = {l} should be well under 0.03 L_sun");
    assert!(l > 0.01, "L(M/3) = {l} should be over 0.01 L_sun");
}

#[test]
fn constants_are_the_published_values() {
    close(G, 6.6743e-11, 1e-6);
    close(AU_M, 1.496e11, 1e-3);
    close(SOLAR_MASS_KG, 1.989e30, 1e-3);
}
