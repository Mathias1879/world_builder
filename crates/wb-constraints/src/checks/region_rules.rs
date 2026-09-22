use crate::checker::{CheckContext, Checker, ConstraintView, KindPattern};
use crate::geo::{centroid, contains};
use crate::kinds::{polygon, shape, text};
use crate::model::{Finding, Param};

/// Features a region rule forbids should not lie inside it.
pub struct RegionRulesCheck;

fn forbidden(rule: &str) -> &'static [&'static str] {
    match rule {
        "no_rain" => &["feature.lake", "feature.river", "feature.forest"],
        "permanent_ice" => &["feature.forest", "feature.desert", "civ.settlement"],
        _ => &[],
    }
}

impl Checker for RegionRulesCheck {
    fn id(&self) -> &'static str {
        "static.region_rules"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("rule.region")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let (Some(ring), Some(rule)) = (polygon(&c, "area"), text(&c, "rule")) else {
            return Vec::new();
        };
        let banned = forbidden(rule);
        let mut out = Vec::new();
        for other in ctx.state.live() {
            let Some(kind) = other.kind() else { continue };
            if other.id == c.id || !banned.contains(&kind) {
                continue;
            }
            let Some(center) = shape(&other).and_then(|(pts, _)| centroid(&pts)) else {
                continue;
            };
            if contains(ring, center) {
                out.push(
                    Finding::issue(self.id(), "rule.forbidden_feature", 0.6)
                        .with_param("feature_kind", Param::Text(kind.to_string()))
                        .with_param("rule", Param::Text(rule.to_string()))
                        .with_related(other.id),
                );
            }
        }
        out
    }
}
