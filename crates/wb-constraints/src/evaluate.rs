use crate::checker::{CheckContext, CheckerRegistry};
use crate::grade::{effective_realism, make_verdict};
use crate::kinds::{float, is_constraint_kind};
use crate::model::{Finding, FindingKind, Preset, Report};
use std::collections::BTreeSet;
use wb_editlog::{EntityId, State, Value};

/// The world's realism slider: planet `realism` (clamped to [0, 1]), else Plausible fantasy.
pub fn realism_of(state: &State) -> f64 {
    match state.field(EntityId::PLANET, "realism") {
        Some(Value::Float(r)) => r.clamp(0.0, 1.0),
        _ => Preset::PlausibleFantasy.value(),
    }
}

fn sort_key(f: &Finding) -> (String, String, Vec<u8>) {
    (
        f.checker.clone(),
        f.code.0.clone(),
        postcard::to_allocvec(&f.params).expect("params always serialize"),
    )
}

/// Evaluates every live constraint with every matching checker.
///
/// Live constraints are visited in `EntityId` order and matching checkers run in
/// checker-id order; findings are then sorted by `(checker, code, params)`, so the
/// resulting [`Report`] — and its hash — is fully determined by the state.
///
/// # Badness normalization
///
/// Checkers are pure but not trusted to stay inside the contract, so `evaluate`
/// normalizes every finding's `badness` before grading (spec §5.4 mentions only the
/// clamp; this is the full rule):
///
/// - `FindingKind::Issue` with NaN badness becomes `1.0` — an unreadable badness is
///   treated as the worst case rather than poisoning the `max`-fold behind the score.
/// - Any other `Issue` badness is clamped into `[0, 1]`.
/// - `Support` and `Consequence` badness is forced to `0.0`, whatever the checker set,
///   so a non-issue can never move the score.
///
/// This is the only place normalization happens: [`make_verdict`] assumes it is
/// already done.
pub fn evaluate(ctx: &CheckContext<'_>, registry: &CheckerRegistry) -> Report {
    let mut verdicts = Vec::new();
    for view in ctx.state.live() {
        let Some(kind) = view.kind() else { continue };
        if !is_constraint_kind(kind) {
            continue;
        }
        let mut findings: Vec<Finding> = registry
            .iter()
            .filter(|c| c.kinds().iter().any(|p| p.matches(kind)))
            .flat_map(|c| c.check(view, ctx))
            .collect();
        for f in &mut findings {
            f.badness = match f.kind {
                FindingKind::Issue if f.badness.is_nan() => 1.0,
                FindingKind::Issue => f.badness.clamp(0.0, 1.0),
                _ => 0.0,
            };
        }
        findings.sort_by_cached_key(sort_key);
        let tolerance = float(&view, "tolerance").unwrap_or(0.0);
        let keep: BTreeSet<String> = match view.get("keep_codes") {
            Some(Value::List(items)) => items
                .iter()
                .filter_map(|v| match v {
                    Value::Text(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            _ => BTreeSet::new(),
        };
        verdicts.push(make_verdict(
            view.id,
            kind,
            findings,
            &keep,
            effective_realism(ctx.realism, tolerance),
        ));
    }
    Report::new(verdicts)
}
