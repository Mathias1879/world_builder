//! Starter checkers that need no simulation.

mod coast;
mod lore_area;
mod relative_position;
mod river_mouth;

pub use coast::CoastCheck;
pub use lore_area::{LoreAreaCheck, MI2_IN_KM2};
pub use relative_position::RelativePositionCheck;
pub use river_mouth::RiverMouthCheck;
