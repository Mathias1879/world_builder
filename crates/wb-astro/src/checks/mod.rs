//! Feasibility checkers (spec §6). Every finding carries the numbers that produced it,
//! and every issue can be kept anyway like any other constraint.

mod calendar;
mod orbital;
mod world;

pub use calendar::CalendarCheck;
pub use orbital::{
    BeltCheck, CompanionCheck, MoonCheck, OrbitCheck, OrderingCheck, critical_distance_m,
    critical_fraction, critical_fraction_outer, hill_radius_m, roche_limit_m, simple_ratio,
};
pub use world::{
    BodyCheck, ClimateCheck, HABITABLE_K, SpinCheck, equilibrium_temperature_k, locking_time_s,
    mean_stellar_flux,
};

use std::collections::BTreeMap;
use wb_constraints::{Checker, CheckerRegistry, IssueCode, Param, Suggestion};
use wb_editlog::{EntityId, Value};

/// Builds a [`Suggestion`] from its code, its display params, and the writes that clear
/// the finding it is attached to.
pub(crate) fn suggestion(
    code: &str,
    params: Vec<(&str, Param)>,
    writes: Vec<(EntityId, String, Value)>,
) -> Suggestion {
    Suggestion {
        code: IssueCode::new(code),
        params: params
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect::<BTreeMap<String, Param>>(),
        writes,
        creates: Vec::new(),
    }
}

/// Registers all nine astro checkers.
pub fn with_astro_checks(registry: &mut CheckerRegistry) {
    for c in [
        Box::new(BeltCheck) as Box<dyn Checker>,
        Box::new(BodyCheck),
        Box::new(CalendarCheck),
        Box::new(ClimateCheck),
        Box::new(CompanionCheck),
        Box::new(MoonCheck),
        Box::new(OrbitCheck),
        Box::new(OrderingCheck),
        Box::new(SpinCheck),
    ] {
        registry.register(c).expect("astro checker ids are unique");
    }
}
