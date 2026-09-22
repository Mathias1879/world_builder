use crate::extent::Extent;
use crate::feature::Geometry;
use crate::tile::{CellValue, Dtype};
use crate::world::{LayerStore, World};
use wb_grid::{LatLon, TileId};

/// Digest of the edit-log state a world was generated from (subsystem [2]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceHash(pub [u8; 32]);

fn put_str(h: &mut blake3::Hasher, s: &str) {
    h.update(&(s.len() as u64).to_le_bytes());
    h.update(s.as_bytes());
}

fn put_tile_id(h: &mut blake3::Hasher, t: TileId) {
    h.update(&[t.face.index(), t.level]);
    h.update(&t.x.to_le_bytes());
    h.update(&t.y.to_le_bytes());
}

/// Canonical quiet NaN hashed for every f64 NaN (sign and payload vary by target).
const CANONICAL_NAN_F64: u64 = 0x7FF8_0000_0000_0000;

fn put_f64(h: &mut blake3::Hasher, v: f64) {
    let bits = if v.is_nan() {
        CANONICAL_NAN_F64
    } else {
        v.to_bits()
    };
    h.update(&bits.to_le_bytes());
}

fn put_points(h: &mut blake3::Hasher, pts: &[LatLon]) {
    h.update(&(pts.len() as u64).to_le_bytes());
    for p in pts {
        put_f64(h, p.lat);
        put_f64(h, p.lon);
    }
}

/// Content address of one generated tile.
pub fn tile_cache_key(
    source: &SourceHash,
    seed: u64,
    engine_version: &str,
    layer_name: &str,
    tile: TileId,
) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(b"wb-tile-key-v1");
    h.update(&source.0);
    h.update(&seed.to_le_bytes());
    put_str(&mut h, engine_version);
    put_str(&mut h, layer_name);
    put_tile_id(&mut h, tile);
    h.finalize().into()
}

fn put_store<T: CellValue>(h: &mut blake3::Hasher, store: &LayerStore<T>) {
    h.update(&(store.len() as u64).to_le_bytes());
    for (id, tile) in store.iter() {
        put_tile_id(h, *id);
        h.update(&tile.content_hash());
    }
}

/// Digest of an entire world, independent of platform.
pub fn world_hash(world: &World) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(b"wb-world-v1");
    put_f64(&mut h, world.planet.radius_m);
    match world.extent {
        Extent::Planet => {
            h.update(&[0]);
        }
        Extent::Region(r) => {
            h.update(&[1]);
            for v in [r.south, r.north, r.west, r.east] {
                put_f64(&mut h, v);
            }
        }
    }
    for (id, d) in world.registry().iter() {
        put_str(&mut h, &d.name);
        h.update(&[d.dtype as u8, d.lod as u8]);
        put_str(&mut h, &d.unit);
        put_str(&mut h, &d.producer);
        match d.dtype {
            Dtype::F32 => put_store(&mut h, world.layer::<f32>(id).expect("registered")),
            Dtype::U8 => put_store(&mut h, world.layer::<u8>(id).expect("registered")),
            Dtype::U16 => put_store(&mut h, world.layer::<u16>(id).expect("registered")),
            Dtype::I16 => put_store(&mut h, world.layer::<i16>(id).expect("registered")),
        }
    }
    h.update(&(world.features.len() as u64).to_le_bytes());
    for f in world.features.iter() {
        h.update(&f.id.0.to_le_bytes());
        put_str(&mut h, &f.kind);
        match &f.geometry {
            Geometry::Point(p) => {
                h.update(&[0]);
                put_points(&mut h, core::slice::from_ref(p));
            }
            Geometry::LineString(pts) => {
                h.update(&[1]);
                put_points(&mut h, pts);
            }
            Geometry::Polygon(pts) => {
                h.update(&[2]);
                put_points(&mut h, pts);
            }
        }
    }
    h.finalize().into()
}

pub fn to_hex(h: &[u8; 32]) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}
