//! Astronomy & Calendar: a planetary system's numbers, its calendar, and the feasibility
//! checkers that judge both.

mod error;
mod units;

pub use error::AstroError;
pub use units::{
    AU_M, BodyClass, ClassDefaults, EARTH_GRAVITY_MS2, EARTH_RADIUS_KM, G, MIN_STELLAR_MASS_SOLAR,
    SECONDS_PER_HOUR, SOLAR_CONSTANT_W, SOLAR_LUMINOSITY_W, SOLAR_MASS_KG, STEFAN_BOLTZMANN,
    gravity_from_mass, luminosity_from_mass_solar, mass_from_density, mass_from_gravity,
};
