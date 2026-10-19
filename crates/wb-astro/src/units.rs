//! Physical constants and per-class defaults. SI throughout: metres, kilograms, seconds,
//! watts, kelvin. User-facing fields use km, AU, days and hours; conversion happens where
//! the field is read, never deeper in.

/// Gravitational constant, m³·kg⁻¹·s⁻².
pub const G: f64 = 6.674_30e-11;
/// Astronomical unit, metres.
pub const AU_M: f64 = 1.495_978_707e11;
pub const SOLAR_MASS_KG: f64 = 1.988_41e30;
pub const SOLAR_LUMINOSITY_W: f64 = 3.828e26;
/// Stefan–Boltzmann constant, W·m⁻²·K⁻⁴.
pub const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;
pub const SECONDS_PER_HOUR: f64 = 3_600.0;
/// Default home-world surface gravity when the planet entity does not state one.
pub const EARTH_GRAVITY_MS2: f64 = 9.806_65;
/// Default home-world radius when the planet entity does not state one.
pub const EARTH_RADIUS_KM: f64 = 6_371.0;
/// Flux reference for `apparent_brightness`: Earth's solar constant, W·m⁻².
pub const SOLAR_CONSTANT_W: f64 = 1_361.0;
/// Below this mass an object cannot fuse hydrogen, so it is not a star.
pub const MIN_STELLAR_MASS_SOLAR: f64 = 0.08;

/// What a body is. The first six parse from an `astro.body` `class` field; `Star` and
/// `Home` are assigned by the system builder to `astro.star` entities and the planet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BodyClass {
    Rocky,
    Water,
    GasGiant,
    IceGiant,
    Belt,
    Moon,
    Star,
    Home,
}

/// Stand-in physical properties for a body the user described only by class (spec §3.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClassDefaults {
    pub radius_km: f64,
    pub density_kgm3: f64,
    pub albedo: f64,
}

impl BodyClass {
    /// Parses a `class` field. Returns `None` for anything not in spec §3.1.
    pub fn parse(s: &str) -> Option<BodyClass> {
        Some(match s {
            "rocky" => BodyClass::Rocky,
            "water" => BodyClass::Water,
            "gas_giant" => BodyClass::GasGiant,
            "ice_giant" => BodyClass::IceGiant,
            "belt" => BodyClass::Belt,
            "moon" => BodyClass::Moon,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            BodyClass::Rocky => "rocky",
            BodyClass::Water => "water",
            BodyClass::GasGiant => "gas_giant",
            BodyClass::IceGiant => "ice_giant",
            BodyClass::Belt => "belt",
            BodyClass::Moon => "moon",
            BodyClass::Star => "star",
            BodyClass::Home => "home",
        }
    }

    /// Whether this class has a solid surface a moon could orbit meaningfully.
    pub fn is_belt(self) -> bool {
        self == BodyClass::Belt
    }

    pub fn defaults(self) -> ClassDefaults {
        let (radius_km, density_kgm3, albedo) = match self {
            BodyClass::Rocky => (5_000.0, 5_000.0, 0.15),
            BodyClass::Water => (6_500.0, 3_000.0, 0.30),
            BodyClass::GasGiant => (60_000.0, 1_300.0, 0.50),
            BodyClass::IceGiant => (25_000.0, 1_600.0, 0.50),
            // A belt is not a body: its radius and density are placeholders so arithmetic
            // never divides by zero, and only `albedo` is used.
            BodyClass::Belt => (1.0, 1_000.0, 0.10),
            BodyClass::Moon => (1_500.0, 3_300.0, 0.12),
            BodyClass::Star => (695_700.0, 1_408.0, 0.0),
            BodyClass::Home => (EARTH_RADIUS_KM, 5_514.0, 0.30),
        };
        ClassDefaults {
            radius_km,
            density_kgm3,
            albedo,
        }
    }
}

/// `M = g·R²/G` — the home world's mass from its surface gravity.
pub fn mass_from_gravity(gravity_ms2: f64, radius_m: f64) -> f64 {
    gravity_ms2 * radius_m * radius_m / G
}

/// A uniform sphere's mass.
pub fn mass_from_density(radius_m: f64, density_kgm3: f64) -> f64 {
    let volume = 4.0 / 3.0 * std::f64::consts::PI * radius_m * radius_m * radius_m;
    volume * density_kgm3
}

/// The main-sequence mass–luminosity relation, `L = M^3.5`, in solar units (spec §3.2).
pub fn luminosity_from_mass_solar(mass_solar: f64) -> f64 {
    libm::pow(mass_solar, 3.5)
}

/// Surface gravity from mass and radius — the inverse of [`mass_from_gravity`].
pub fn gravity_from_mass(mass_kg: f64, radius_m: f64) -> f64 {
    G * mass_kg / (radius_m * radius_m)
}
