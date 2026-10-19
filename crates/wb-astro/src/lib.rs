//! Astronomy & Calendar: a planetary system's numbers, its calendar, and the feasibility
//! checkers that judge both.

mod calendar;
mod checks;
mod error;
mod kepler;
mod kinds;
mod seasons;
mod sky;
mod system;
mod units;

pub use calendar::{CalendarModel, Date, MonthView, SeasonView};
pub use checks::{
    BeltCheck, CompanionCheck, MoonCheck, OrbitCheck, OrderingCheck, critical_distance_m,
    critical_fraction, hill_radius_m, roche_limit_m, simple_ratio, with_astro_checks,
};
pub use error::AstroError;
pub use kepler::{
    KEPLER_ITERATIONS, eccentric_anomaly, ecliptic_longitude, mean_anomaly, orbit_position,
    period_s, radius_m, semi_major_m, true_anomaly, wrap_angle,
};
pub use kinds::{ASTRO_KINDS, boolean, int, number, register_catalog, register_kinds, words};
pub use seasons::{PROPOSAL_LAT_DEG, season_cuts};
pub use sky::{SkyModel, daily_mean_factor, hour_angle, tidal_amplitude_m};
pub use system::{BodyView, DEFAULT_DAY_HOURS, DEFAULT_TILT_DEG, DEFAULT_YEAR_DAYS, HomeSettings};
pub use units::{
    AU_M, BodyClass, ClassDefaults, EARTH_GRAVITY_MS2, EARTH_RADIUS_KM, G, MIN_STELLAR_MASS_SOLAR,
    SECONDS_PER_HOUR, SOLAR_CONSTANT_W, SOLAR_LUMINOSITY_W, SOLAR_MAGNITUDE, SOLAR_MASS_KG,
    STEFAN_BOLTZMANN, gravity_from_mass, luminosity_from_mass_solar, mass_from_density,
    mass_from_gravity,
};
