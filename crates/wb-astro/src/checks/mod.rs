//! Feasibility checkers (spec §6). Every finding carries the numbers that produced it,
//! and every issue can be kept anyway like any other constraint.

mod orbital;

pub use orbital::{
    BeltCheck, CompanionCheck, MoonCheck, OrbitCheck, OrderingCheck, critical_distance_m,
    critical_fraction, hill_radius_m, roche_limit_m, simple_ratio,
};

use wb_constraints::{Checker, CheckerRegistry};

/// Registers every astro checker.
pub fn with_astro_checks(registry: &mut CheckerRegistry) {
    for c in [
        Box::new(BeltCheck) as Box<dyn Checker>,
        Box::new(CompanionCheck),
        Box::new(MoonCheck),
        Box::new(OrbitCheck),
        Box::new(OrderingCheck),
    ] {
        registry.register(c).expect("astro checker ids are unique");
    }
}
