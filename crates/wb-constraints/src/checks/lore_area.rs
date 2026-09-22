use crate::checker::{CheckContext, Checker, ConstraintView, KindPattern};
use crate::kinds::{entity, float, polygon, text};
use crate::model::{Finding, IssueCode, Param, Suggestion};
use wb_editlog::Value;
use wb_world::polygon_area_m2;

pub const MI2_IN_KM2: f64 = 2.589988110336;

/// A lore-stated area should match the drawn area.
pub struct LoreAreaCheck;

impl Checker for LoreAreaCheck {
    fn id(&self) -> &'static str {
        "static.lore_area"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("lore.area")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let (Some(subject), Some(stated), Some(unit)) =
            (entity(&c, "subject"), float(&c, "value"), text(&c, "unit"))
        else {
            return Vec::new();
        };
        let factor = match unit {
            "km2" => 1.0,
            "mi2" => MI2_IN_KM2,
            _ => return Vec::new(),
        };
        let Some(view) = ctx.state.entity(subject).filter(|v| !v.is_deleted()) else {
            return Vec::new();
        };
        let Some(ring) = polygon(&view, "area") else {
            return Vec::new();
        };
        let drawn = polygon_area_m2(&ctx.planet, ring) / 1e6 / factor;
        if drawn <= 0.0 || stated <= 0.0 {
            return Vec::new();
        }
        let ratio = drawn.max(stated) / drawn.min(stated);
        let unit_p = Param::Text(unit.to_string());
        if ratio <= 1.15 {
            return vec![
                Finding::support(self.id(), "lore.area_consistent")
                    .with_param("drawn", Param::Float(drawn))
                    .with_param("unit", unit_p),
            ];
        }
        let suggestion = Suggestion {
            code: IssueCode::new("lore.set_area_to_drawn"),
            params: [
                ("drawn".to_string(), Param::Float(drawn)),
                ("unit".to_string(), unit_p.clone()),
            ]
            .into_iter()
            .collect(),
            writes: vec![(c.id, "value".to_string(), Value::Float(drawn))],
        };
        vec![
            Finding::issue(
                self.id(),
                "lore.area_mismatch",
                ((ratio - 1.15) / 1.85).min(1.0),
            )
            .with_param("drawn", Param::Float(drawn))
            .with_param("stated", Param::Float(stated))
            .with_param("unit", unit_p)
            .with_param("ratio", Param::Float(ratio))
            .with_related(subject)
            .with_suggestion(suggestion),
        ]
    }
}
