use crate::extent::Extent;
use crate::feature::FeatureSet;
use crate::layer::{LayerDesc, LayerId, LayerRegistry, RegistryError};
use crate::tile::{CellValue, Dtype, Tile};
use core::fmt;
use std::collections::BTreeMap;
use wb_grid::{CellId, TileId};

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Planet {
    pub radius_m: f64,
}

impl Default for Planet {
    fn default() -> Self {
        Self {
            radius_m: 6_371_000.0,
        }
    }
}

impl Planet {
    pub fn cell_area_m2(&self, c: CellId) -> f64 {
        c.area_unit() * self.radius_m * self.radius_m
    }
}

/// All tiles of one layer, keyed and iterated in `TileId` order.
#[derive(Clone, Debug)]
pub struct LayerStore<T: CellValue> {
    tiles: BTreeMap<TileId, Tile<T>>,
}

impl<T: CellValue> Default for LayerStore<T> {
    fn default() -> Self {
        Self {
            tiles: BTreeMap::new(),
        }
    }
}

impl<T: CellValue> LayerStore<T> {
    pub fn insert(&mut self, tile: Tile<T>) -> Option<Tile<T>> {
        self.tiles.insert(tile.id(), tile)
    }

    pub fn get(&self, id: TileId) -> Option<&Tile<T>> {
        self.tiles.get(&id)
    }

    pub fn get_mut(&mut self, id: TileId) -> Option<&mut Tile<T>> {
        self.tiles.get_mut(&id)
    }

    pub fn cell(&self, c: CellId) -> Option<T> {
        let (tile, i, j) = c.tile();
        self.tiles.get(&tile).map(|t| t.get(i, j))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&TileId, &Tile<T>)> {
        self.tiles.iter()
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorldError {
    Registry(RegistryError),
    UnknownLayer(LayerId),
    DtypeMismatch { layer: Dtype, requested: Dtype },
}

impl fmt::Display for WorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorldError::Registry(e) => write!(f, "{e}"),
            WorldError::UnknownLayer(id) => write!(f, "unknown layer {id:?}"),
            WorldError::DtypeMismatch { layer, requested } => {
                write!(f, "layer holds {layer:?} but {requested:?} was requested")
            }
        }
    }
}

impl std::error::Error for WorldError {}

/// The generated world: planet, extent, and typed per-cell layers.
#[derive(Clone, Debug)]
pub struct World {
    pub planet: Planet,
    pub extent: Extent,
    pub features: FeatureSet,
    registry: LayerRegistry,
    f32_layers: BTreeMap<LayerId, LayerStore<f32>>,
    u8_layers: BTreeMap<LayerId, LayerStore<u8>>,
    u16_layers: BTreeMap<LayerId, LayerStore<u16>>,
    i16_layers: BTreeMap<LayerId, LayerStore<i16>>,
}

/// Cell types a `World` can store.
pub trait WorldValue: CellValue {
    #[doc(hidden)]
    fn stores(w: &World) -> &BTreeMap<LayerId, LayerStore<Self>>;
    #[doc(hidden)]
    fn stores_mut(w: &mut World) -> &mut BTreeMap<LayerId, LayerStore<Self>>;
}

macro_rules! world_value {
    ($t:ty, $field:ident) => {
        impl WorldValue for $t {
            fn stores(w: &World) -> &BTreeMap<LayerId, LayerStore<Self>> {
                &w.$field
            }
            fn stores_mut(w: &mut World) -> &mut BTreeMap<LayerId, LayerStore<Self>> {
                &mut w.$field
            }
        }
    };
}

world_value!(f32, f32_layers);
world_value!(u8, u8_layers);
world_value!(u16, u16_layers);
world_value!(i16, i16_layers);

impl World {
    pub fn new(planet: Planet, extent: Extent) -> Self {
        Self {
            planet,
            extent,
            features: FeatureSet::default(),
            registry: LayerRegistry::default(),
            f32_layers: BTreeMap::new(),
            u8_layers: BTreeMap::new(),
            u16_layers: BTreeMap::new(),
            i16_layers: BTreeMap::new(),
        }
    }

    pub fn registry(&self) -> &LayerRegistry {
        &self.registry
    }

    /// Registers a layer and creates its empty store.
    pub fn register_layer(&mut self, desc: LayerDesc) -> Result<LayerId, WorldError> {
        let dtype = desc.dtype;
        let id = self.registry.register(desc).map_err(WorldError::Registry)?;
        match dtype {
            Dtype::F32 => {
                self.f32_layers.insert(id, LayerStore::default());
            }
            Dtype::U8 => {
                self.u8_layers.insert(id, LayerStore::default());
            }
            Dtype::U16 => {
                self.u16_layers.insert(id, LayerStore::default());
            }
            Dtype::I16 => {
                self.i16_layers.insert(id, LayerStore::default());
            }
        }
        Ok(id)
    }

    fn check<T: WorldValue>(&self, id: LayerId) -> Result<(), WorldError> {
        let desc = self.registry.get(id).ok_or(WorldError::UnknownLayer(id))?;
        if desc.dtype != T::DTYPE {
            return Err(WorldError::DtypeMismatch {
                layer: desc.dtype,
                requested: T::DTYPE,
            });
        }
        Ok(())
    }

    pub fn layer<T: WorldValue>(&self, id: LayerId) -> Result<&LayerStore<T>, WorldError> {
        self.check::<T>(id)?;
        Ok(T::stores(self)
            .get(&id)
            .expect("registered layers always have a store"))
    }

    pub fn layer_mut<T: WorldValue>(
        &mut self,
        id: LayerId,
    ) -> Result<&mut LayerStore<T>, WorldError> {
        self.check::<T>(id)?;
        Ok(T::stores_mut(self)
            .get_mut(&id)
            .expect("registered layers always have a store"))
    }
}
