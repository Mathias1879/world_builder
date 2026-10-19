//! Whether the calendar matches the orbit it is supposed to describe.

use crate::calendar::MonthView;
use crate::checks::orbital::checker_model;
use crate::checks::suggestion;
use std::collections::BTreeMap;
use wb_constraints::{CheckContext, Checker, ConstraintView, Finding, KindPattern, Param};
use wb_editlog::Value;

/// Drift above this many days per year is worth saying out loud.
const DRIFT_TOLERANCE_DAYS: f64 = 0.01;
/// Above half a day, the months simply do not add up.
const SUM_TOLERANCE_DAYS: f64 = 0.5;

/// Months against `year_days`, and the leap rule's residual drift.
pub struct CalendarCheck;

impl Checker for CalendarCheck {
    fn id(&self) -> &'static str {
        "astro.calendar"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("astro.calendar")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = checker_model(ctx) else {
            return Vec::new();
        };
        let cal = sky.calendar();
        if cal.id != c.id {
            // A second calendar is not the one the model uses; judging it would need a
            // model of its own, and the world has one calendar by design.
            return Vec::new();
        }
        let mut out = Vec::new();

        // Two or more months sharing an `index` is a cross-entity property no
        // per-entity validator can catch, and it silently corrupts the date round-trip
        // spec §4.5 promises: `month_position_by_index` takes the first match while
        // `date_of` emits whichever position the day actually falls in, so the two stop
        // agreeing for every date that lands on the duplicated month. No suggestion —
        // which index should move is the user's call, and this branch's principle is
        // that a fix which cannot be chosen for the user is better left unoffered.
        let mut by_index: BTreeMap<i64, Vec<&MonthView>> = BTreeMap::new();
        for m in &cal.months {
            by_index.entry(m.index).or_default().push(m);
        }
        for (index, group) in &by_index {
            if group.len() < 2 {
                continue;
            }
            let names = group
                .iter()
                .map(|m| m.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let mut finding = Finding::issue(self.id(), "astro.month_index_duplicated", 0.9)
                .with_param("index", Param::Int(*index))
                .with_param("months", Param::Text(names));
            for m in group {
                finding = finding.with_related(m.id);
            }
            out.push(finding);
        }

        let months = cal.month_days() as f64;
        // Mean calendar year: the months, plus a leap day every n years.
        let mean = months
            + match cal.leap_every_n {
                Some(n) if n >= 2 => 1.0 / n as f64,
                _ => 0.0,
            };
        let year_days = sky.year_days();
        let drift = mean - year_days;
        if drift.abs() > SUM_TOLERANCE_DAYS {
            // Lengthen (or shorten) the longest month to absorb the difference.
            let longest = cal
                .months
                .iter()
                .max_by_key(|m| (m.days, std::cmp::Reverse(m.id)));
            let mut finding = Finding::issue(self.id(), "astro.calendar_sum_mismatch", 0.9)
                .with_param("month_total", Param::Float(months))
                .with_param("year_days", Param::Float(year_days))
                .with_param("difference_days", Param::Float(-drift));
            if let Some(m) = longest {
                let adjusted = m.days - libm::round(drift) as i64;
                if adjusted >= 1 {
                    finding = finding.with_suggestion(suggestion(
                        "astro.adjust_longest_month",
                        vec![
                            ("month", Param::Text(m.name.clone())),
                            ("days", Param::Int(adjusted)),
                        ],
                        vec![(m.id, "days".to_string(), Value::Int(adjusted))],
                    ));
                }
                finding = finding.with_related(m.id);
            }
            out.push(finding);
            return out;
        }
        if drift.abs() > DRIFT_TOLERANCE_DAYS {
            out.push(
                Finding::issue(self.id(), "astro.leap_drift", 0.6)
                    .with_param("drift_days", Param::Float(drift.abs()))
                    .with_param("mean_year_days", Param::Float(mean))
                    .with_param("year_days", Param::Float(year_days)),
            );
            return out;
        }
        out
    }
}
