use crate::checker::{CheckContext, Checker, ConstraintView, KindPattern};
use crate::geo::centroid;
use crate::kinds::{float, shape, text};
use crate::model::{Finding, Param};

/// Gentle latitude sanity checks; superseded by the climate stage later.
pub struct LatitudeSanityCheck;

impl Checker for LatitudeSanityCheck {
    fn id(&self) -> &'static str {
        "static.latitude_sanity"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[
            KindPattern::Exact("feature.glacier"),
            KindPattern::Exact("feature.forest"),
        ]
    }

    fn check(&self, c: ConstraintView<'_>, _ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(center) = shape(&c).and_then(|(pts, _)| centroid(&pts)) else {
            return Vec::new();
        };
        let lat = center.lat.to_degrees();
        let issue = |code: &str, badness: f64| {
            vec![Finding::issue(self.id(), code, badness).with_param("lat", Param::Float(lat))]
        };
        match (c.kind(), text(&c, "biome")) {
            (Some("feature.glacier"), _)
                if float(&c, "elevation_m").unwrap_or(0.0) < 500.0 && lat.abs() <= 25.0 =>
            {
                issue("climate.lowland_tropical_glacier", 0.6)
            }
            (Some("feature.forest"), Some("tropical")) if lat.abs() > 35.0 => {
                issue("climate.tropical_forest_high_latitude", 0.5)
            }
            (Some("feature.forest"), Some("boreal")) if lat.abs() < 30.0 => {
                issue("climate.boreal_forest_low_latitude", 0.4)
            }
            _ => Vec::new(),
        }
    }
}
