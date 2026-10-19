use std::fmt;
use wb_editlog::EntityId;

/// Why a system could not be read at all. Everything merely implausible is a verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AstroError {
    /// No `astro.star` has `role = primary`.
    NoPrimaryStar,
    /// More than one `astro.star` has `role = primary`.
    MultiplePrimaryStars,
    /// A body names a `parent` that is not a live body or star.
    MissingParent(EntityId),
    /// Following `parent` links from this body never reaches the primary star.
    ParentCycle(EntityId),
    /// No `astro.calendar` entity.
    NoCalendar,
    /// The calendar has no `astro.month` entities.
    NoMonths,
}

impl fmt::Display for AstroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AstroError::NoPrimaryStar => write!(f, "no star has role = primary"),
            AstroError::MultiplePrimaryStars => write!(f, "more than one star has role = primary"),
            AstroError::MissingParent(e) => {
                write!(f, "body #{}.{} orbits an unknown parent", e.op.lamport, e.n)
            }
            AstroError::ParentCycle(e) => {
                write!(f, "body #{}.{} is in a parent cycle", e.op.lamport, e.n)
            }
            AstroError::NoCalendar => write!(f, "the world has no calendar"),
            AstroError::NoMonths => write!(f, "the calendar has no months"),
        }
    }
}

impl std::error::Error for AstroError {}
