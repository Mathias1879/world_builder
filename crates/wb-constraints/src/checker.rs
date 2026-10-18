use crate::error::ConstraintError;
use crate::evaluate::realism_of;
use crate::kinds::KindCatalog;
use crate::model::Finding;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use wb_editlog::{EntityView, State};
use wb_grid::LatLon;
use wb_world::Planet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KindPattern {
    Exact(&'static str),
    Prefix(&'static str),
}

impl KindPattern {
    pub fn matches(&self, kind: &str) -> bool {
        match self {
            KindPattern::Exact(k) => kind == *k,
            KindPattern::Prefix(p) => kind.starts_with(p),
        }
    }
}

/// What checkers may ask about the physical world. `None` = not known yet.
pub trait Terrain: Send + Sync {
    fn is_land(&self, p: LatLon) -> Option<bool>;
}

/// Terrain before any map is imported or generated.
pub struct UnknownTerrain;

impl Terrain for UnknownTerrain {
    fn is_land(&self, _p: LatLon) -> Option<bool> {
        None
    }
}

/// The starter catalog, so `CheckContext::new` can hand out a `&'static KindCatalog`.
fn starter_catalog() -> &'static KindCatalog {
    static STARTER: OnceLock<KindCatalog> = OnceLock::new();
    STARTER.get_or_init(KindCatalog::with_starter_kinds)
}

pub struct CheckContext<'a> {
    pub state: &'a State,
    pub planet: Planet,
    pub realism: f64,
    pub terrain: &'a dyn Terrain,
    /// The kinds [`crate::evaluate`] will judge.
    pub catalog: &'a KindCatalog,
}

impl<'a> CheckContext<'a> {
    /// A context over the starter kinds only.
    pub fn new(state: &'a State, planet: Planet, terrain: &'a dyn Terrain) -> Self {
        CheckContext::with_catalog(state, planet, terrain, starter_catalog())
    }

    /// A context over `catalog` — how a subsystem gets its own kinds evaluated.
    pub fn with_catalog(
        state: &'a State,
        planet: Planet,
        terrain: &'a dyn Terrain,
        catalog: &'a KindCatalog,
    ) -> Self {
        CheckContext {
            state,
            planet,
            realism: realism_of(state),
            terrain,
            catalog,
        }
    }
}

pub type ConstraintView<'a> = EntityView<'a>;

/// A pure, deterministic check over one constraint.
pub trait Checker: Send + Sync {
    fn id(&self) -> &'static str;
    fn kinds(&self) -> &[KindPattern];
    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding>;
}

#[derive(Default)]
pub struct CheckerRegistry {
    checkers: BTreeMap<&'static str, Box<dyn Checker>>,
}

impl CheckerRegistry {
    pub fn new() -> Self {
        CheckerRegistry::default()
    }

    pub fn register(&mut self, checker: Box<dyn Checker>) -> Result<(), ConstraintError> {
        let id = checker.id();
        if self.checkers.contains_key(id) {
            return Err(ConstraintError::DuplicateChecker(id.to_string()));
        }
        self.checkers.insert(id, checker);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.checkers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checkers.is_empty()
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.checkers.keys().copied().collect()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &dyn Checker> {
        self.checkers.values().map(|c| c.as_ref())
    }
}
