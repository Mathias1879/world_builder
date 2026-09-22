//! World data model: typed per-cell layers in tiles, LOD, extents, features, hashing.

mod apron;
mod extent;
mod feature;
mod geometry;
mod layer;
mod lod;
mod tile;
mod world;

pub use apron::{APRON_EDGE, ApronError, apron_index, gather_with_apron};
pub use extent::{Extent, ExtentError, RegionBox};
pub use feature::{Feature, FeatureId, FeatureSet, Geometry};
pub use geometry::{central_angle, distance_m, polygon_area_m2, polyline_length_m};
pub use layer::{LayerDesc, LayerId, LayerRegistry, LodPolicy, RegistryError};
pub use lod::{Aggregate, LodError, downsample, refine_cell, refine_tile};
pub use tile::{CellValue, DecodeError, Dtype, TILE_CELLS, Tile};
pub use world::{ENGINE_VERSION, LayerStore, Planet, World, WorldError, WorldValue};
