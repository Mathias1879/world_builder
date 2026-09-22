//! Spherical grid: an equi-angular cube-sphere split into quadtree tiles.

mod cell;
mod face;
mod latlon;
mod vec3;

pub use cell::{CellId, MAX_LEVEL, TILE_SIZE, TileId, cells_per_edge, tiles_per_edge};
pub use face::{Face, face_to_sphere, sphere_to_face};
pub use latlon::LatLon;
pub use vec3::Vec3;
