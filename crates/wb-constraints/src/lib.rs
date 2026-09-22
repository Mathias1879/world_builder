//! Constraints & Feasibility: checkers, graded verdicts, keep-anyway, suggestions.

mod error;
mod geo;
mod grade;
mod model;

pub use error::ConstraintError;
pub use geo::{centroid, contains, midpoint, samples};
pub use grade::{effective_realism, grade, make_verdict};
pub use model::{
    Counts, Finding, FindingKind, Grade, IssueCode, Param, Preset, Report, Status, Suggestion,
    Verdict,
};
