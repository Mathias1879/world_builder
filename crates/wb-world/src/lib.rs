//! World data model: typed per-cell layers in tiles, LOD, extents, features, hashing.

mod extent;
mod layer;
mod tile;
mod world;

pub use extent::{Extent, ExtentError, RegionBox};
pub use layer::{LayerDesc, LayerId, LayerRegistry, LodPolicy, RegistryError};
pub use tile::{CellValue, DecodeError, Dtype, TILE_CELLS, Tile};
pub use world::{ENGINE_VERSION, LayerStore, Planet, World, WorldError, WorldValue};
