//! Whether the calendar matches the orbit it is supposed to describe.

use crate::checks::orbital::checker_model;
use crate::checks::suggestion;
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
            return vec![finding];
        }
        if drift.abs() > DRIFT_TOLERANCE_DAYS {
            return vec![
                Finding::issue(self.id(), "astro.leap_drift", 0.6)
                    .with_param("drift_days", Param::Float(drift.abs()))
                    .with_param("mean_year_days", Param::Float(mean))
                    .with_param("year_days", Param::Float(year_days)),
            ];
        }
        Vec::new()
    }
}
