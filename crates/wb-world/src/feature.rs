use std::collections::BTreeMap;
use wb_grid::LatLon;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeatureId(pub u64);

/// Resolution-independent geometry in latitude/longitude.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Geometry {
    Point(LatLon),
    LineString(Vec<LatLon>),
    /// Open ring: the first point is not repeated at the end.
    Polygon(Vec<LatLon>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    pub id: FeatureId,
    pub kind: String,
    pub geometry: Geometry,
}

#[derive(Clone, Debug, Default)]
pub struct FeatureSet {
    features: BTreeMap<FeatureId, Feature>,
}

impl FeatureSet {
    pub fn insert(&mut self, f: Feature) -> Option<Feature> {
        self.features.insert(f.id, f)
    }

    pub fn get(&self, id: FeatureId) -> Option<&Feature> {
        self.features.get(&id)
    }

    pub fn remove(&mut self, id: FeatureId) -> Option<Feature> {
        self.features.remove(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Feature> {
        self.features.values()
    }

    pub fn len(&self) -> usize {
        self.features.len()
    }

    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }
}
