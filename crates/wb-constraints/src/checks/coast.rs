use crate::checker::{CheckContext, Checker, ConstraintView, KindPattern};
use crate::geo::samples;
use crate::kinds::shape;
use crate::model::{Finding, Param};

/// Land features and settlements should not sit over the ocean.
pub struct CoastCheck;

const EXEMPT: &[&str] = &["feature.lake", "feature.river", "feature.glacier"];

impl Checker for CoastCheck {
    fn id(&self) -> &'static str {
        "static.coast"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[
            KindPattern::Prefix("feature."),
            KindPattern::Exact("civ.settlement"),
        ]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        if c.kind().is_some_and(|k| EXEMPT.contains(&k)) {
            return Vec::new();
        }
        let Some((points, closed)) = shape(&c) else {
            return Vec::new();
        };
        let pts = samples(&points, closed);
        let mut ocean = 0usize;
        for p in &pts {
            match ctx.terrain.is_land(*p) {
                None => return Vec::new(),
                Some(false) => ocean += 1,
                Some(true) => {}
            }
        }
        if ocean == 0 || pts.is_empty() {
            return Vec::new();
        }
        let f = ocean as f64 / pts.len() as f64;
        vec![
            Finding::issue(self.id(), "coast.over_ocean", f)
                .with_param("pct", Param::Int((f * 100.0).round() as i64)),
        ]
    }
}
