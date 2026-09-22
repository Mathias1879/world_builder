use crate::model::{Finding, FindingKind, Grade, Status, Verdict};
use std::collections::BTreeSet;
use wb_editlog::EntityId;

/// `clamp(slider + tolerance, 0, 1)`.
pub fn effective_realism(slider: f64, tolerance: f64) -> f64 {
    (slider + tolerance).clamp(0.0, 1.0)
}

/// Plausible if `score ≥ 0.8 − 0.3r`, Implausible if `score < 0.4 − 0.3r`, else Stretch.
pub fn grade(score: f64, r: f64) -> Grade {
    if score >= 0.8 - 0.3 * r {
        Grade::Plausible
    } else if score < 0.4 - 0.3 * r {
        Grade::Implausible
    } else {
        Grade::Stretch
    }
}

/// Partitions findings, applies kept codes, and grades one constraint.
pub fn make_verdict(
    constraint: EntityId,
    kind: &str,
    findings: Vec<Finding>,
    keep: &BTreeSet<String>,
    r: f64,
) -> Verdict {
    let mut issues = Vec::new();
    let mut intentional = Vec::new();
    let mut supports = Vec::new();
    let mut consequences = Vec::new();
    for f in findings {
        match f.kind {
            FindingKind::Support => supports.push(f),
            FindingKind::Consequence => consequences.push(f),
            FindingKind::Issue if keep.contains(f.code.as_str()) => intentional.push(f),
            FindingKind::Issue => issues.push(f),
        }
    }
    let score = 1.0 - issues.iter().map(|f| f.badness).fold(0.0, f64::max);
    let status = if issues.is_empty() && !intentional.is_empty() {
        Status::Intentional
    } else {
        Status::Open
    };
    Verdict {
        constraint,
        kind: kind.to_string(),
        grade: grade(score, r),
        score,
        status,
        issues,
        intentional,
        supports,
        consequences,
    }
}
