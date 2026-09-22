//! Starter checkers that need no simulation.

mod coast;
mod latitude;
mod lore_area;
mod region_rules;
mod relative_position;
mod river_mouth;

pub use coast::CoastCheck;
pub use latitude::LatitudeSanityCheck;
pub use lore_area::{LoreAreaCheck, MI2_IN_KM2};
pub use region_rules::RegionRulesCheck;
pub use relative_position::RelativePositionCheck;
pub use river_mouth::RiverMouthCheck;

use crate::checker::CheckerRegistry;

impl CheckerRegistry {
    /// The six starter checkers (no simulation needed).
    pub fn with_starter_checks() -> CheckerRegistry {
        let mut r = CheckerRegistry::new();
        for c in [
            Box::new(CoastCheck) as Box<dyn crate::checker::Checker>,
            Box::new(LatitudeSanityCheck),
            Box::new(LoreAreaCheck),
            Box::new(RegionRulesCheck),
            Box::new(RelativePositionCheck),
            Box::new(RiverMouthCheck),
        ] {
            r.register(c).expect("starter checker ids are unique");
        }
        r
    }
}
