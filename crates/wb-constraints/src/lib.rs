//! Constraints & Feasibility: checkers, graded verdicts, keep-anyway, suggestions.

mod error;
mod grade;
mod model;

pub use error::ConstraintError;
pub use grade::{effective_realism, grade, make_verdict};
pub use model::{
    Counts, Finding, FindingKind, Grade, IssueCode, Param, Preset, Report, Status, Suggestion,
    Verdict,
};
