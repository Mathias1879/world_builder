//! World data model: typed per-cell layers in tiles, LOD, extents, features, hashing.

mod extent;
mod tile;

pub use extent::{Extent, ExtentError, RegionBox};
pub use tile::{CellValue, DecodeError, Dtype, TILE_CELLS, Tile};
