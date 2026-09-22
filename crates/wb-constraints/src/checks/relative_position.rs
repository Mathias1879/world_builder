use crate::checker::{CheckContext, Checker, ConstraintView, KindPattern};
use crate::geo::centroid;
use crate::kinds::{entity, shape, text};
use crate::model::{Finding, Param};
use wb_editlog::{EntityId, wrap_lon};
use wb_grid::LatLon;

/// "X lies north/south/east/west of Y" should match the drawing.
pub struct RelativePositionCheck;

fn center(ctx: &CheckContext<'_>, id: EntityId) -> Option<LatLon> {
    let view = ctx.state.entity(id).filter(|v| !v.is_deleted())?;
    let (points, _) = shape(&view)?;
    centroid(&points)
}

impl Checker for RelativePositionCheck {
    fn id(&self) -> &'static str {
        "static.relative_position"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("lore.relative_position")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let (Some(s), Some(o), Some(relation)) = (
            entity(&c, "subject"),
            entity(&c, "object"),
            text(&c, "relation"),
        ) else {
            return Vec::new();
        };
        let (Some(sp), Some(op)) = (center(ctx, s), center(ctx, o)) else {
            return Vec::new();
        };
        let dlon = wrap_lon(sp.lon - op.lon);
        // Positive `margin` = satisfied by that many radians; negative = violated.
        let margin = match relation {
            "north_of" => sp.lat - op.lat,
            "south_of" => op.lat - sp.lat,
            "east_of" => dlon,
            "west_of" => -dlon,
            _ => return Vec::new(),
        };
        let rel = Param::Text(relation.to_string());
        if margin > 0.0 {
            return vec![
                Finding::support(self.id(), "lore.relative_position_ok")
                    .with_param("relation", rel),
            ];
        }
        let d = (-margin).to_degrees();
        vec![
            Finding::issue(
                self.id(),
                "lore.relative_position_violated",
                (0.2 + d / 20.0).min(1.0),
            )
            .with_param("relation", rel)
            .with_param("degrees", Param::Float(d))
            .with_related(o),
        ]
    }
}
