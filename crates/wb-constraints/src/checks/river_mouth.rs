use crate::checker::{CheckContext, Checker, ConstraintView, KindPattern};
use crate::geo::{bbox, bbox_contains, contains};
use crate::kinds::{line, polygon};
use crate::model::Finding;
use wb_grid::LatLon;

/// Rivers should reach the sea or a lake, and start on land.
pub struct RiverMouthCheck;

fn in_a_lake(ctx: &CheckContext<'_>, p: LatLon) -> bool {
    ctx.state
        .live()
        .filter(|e| e.kind() == Some("feature.lake"))
        .filter_map(|e| polygon(&e, "area"))
        // Bounding-box prefilter: most lakes are nowhere near this mouth, and the box
        // costs a few comparisons per vertex against the winding test's `atan2`.
        // `bbox` never rejects a point `contains` accepts, so the answer is unchanged.
        .any(|ring| bbox(ring).is_some_and(|b| bbox_contains(b, p)) && contains(ring, p))
}

impl Checker for RiverMouthCheck {
    fn id(&self) -> &'static str {
        "static.river_mouth"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("feature.river")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(path) = line(&c, "path") else {
            return Vec::new();
        };
        let (Some(source), Some(mouth)) = (path.first().copied(), path.last().copied()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if ctx.terrain.is_land(mouth) == Some(true) && !in_a_lake(ctx, mouth) {
            out.push(Finding::issue(self.id(), "river.inland_mouth", 0.3));
        }
        if ctx.terrain.is_land(source) == Some(false) {
            out.push(Finding::issue(self.id(), "river.source_in_ocean", 0.8));
        }
        out
    }
}
