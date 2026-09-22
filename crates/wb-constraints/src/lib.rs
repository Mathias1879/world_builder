//! Constraints & Feasibility: checkers, graded verdicts, keep-anyway, suggestions.

mod actions;
mod checker;
mod error;
mod evaluate;
mod geo;
mod grade;
mod kinds;
mod model;
mod templates;

pub mod checks;

pub use actions::{apply_suggestion, keep_anyway, set_realism, unkeep};
pub use checker::{
    CheckContext, Checker, CheckerRegistry, ConstraintView, KindPattern, Terrain, UnknownTerrain,
};
pub use error::ConstraintError;
pub use evaluate::{evaluate, realism_of};
pub use geo::{centroid, contains, midpoint, samples};
pub use grade::{effective_realism, grade, make_verdict};
pub use kinds::{
    CONSTRAINT_KINDS, entity, float, is_constraint_kind, line, point, polygon, register_kinds,
    shape, text,
};
pub use model::{
    Counts, Finding, FindingKind, Grade, IssueCode, Param, Preset, Report, Status, Suggestion,
    Verdict,
};
pub use templates::{render, render_suggestion};
