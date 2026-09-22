//! Cross-target determinism gate. The same world must hash identically on
//! wasm32-wasip1, x86-64, and ARM64. If this fails after an intentional
//! change to generation or encoding, update GOLDEN in the same commit.

use wb_grid::{CellId, Face, LatLon, TileId, Vec3};
use wb_world::{
    Dtype, Extent, Feature, FeatureId, Geometry, LayerDesc, LodPolicy, Planet, SourceHash, Tile,
    World, refine_tile, tile_cache_key, to_hex, world_hash,
};

const GOLDEN: &str = "58ecf1d9ef5f2c9e307ffac5d7b9f6457fb44c9b03ad2e53c7102cb8f9185e56";

fn pattern(p: Vec3) -> f32 {
    (1000.0 * libm::sin(3.0 * p.x) * libm::cos(2.0 * p.y) + 500.0 * libm::sin(5.0 * p.z)) as f32
}

fn detail(c: CellId) -> f32 {
    let p = c.center();
    (40.0 * libm::sin(40.0 * p.x + 7.0 * p.y) * libm::cos(33.0 * p.z)) as f32
}

fn build() -> World {
    let mut w = World::new(Planet::default(), Extent::Planet);
    let elev = w
        .register_layer(LayerDesc {
            name: "elevation".into(),
            dtype: Dtype::F32,
            unit: "m".into(),
            producer: "golden".into(),
            lod: LodPolicy::AreaMean,
        })
        .unwrap();
    let biome = w
        .register_layer(LayerDesc {
            name: "biome".into(),
            dtype: Dtype::U8,
            unit: "class".into(),
            producer: "golden".into(),
            lod: LodPolicy::Mode,
        })
        .unwrap();
    for t in TileId::all(0) {
        let e = Tile::from_fn(t, |i, j| pattern(t.cell(i, j).center()));
        let b = Tile::from_fn(t, |i, j| ((e.get(i, j) + 2000.0) / 500.0) as u8);
        w.layer_mut::<f32>(elev).unwrap().insert(e);
        w.layer_mut::<u8>(biome).unwrap().insert(b);
    }
    let parent = TileId::new(Face::PosZ, 0, 0, 0).unwrap();
    for c in parent.children().unwrap() {
        let tile = refine_tile(w.layer::<f32>(elev).unwrap(), c, detail).unwrap();
        w.layer_mut::<f32>(elev).unwrap().insert(tile);
    }
    w.features.insert(Feature {
        id: FeatureId(1),
        kind: "region".into(),
        geometry: Geometry::Polygon(vec![
            LatLon::from_degrees(10.0, 10.0),
            LatLon::from_degrees(10.0, 20.0),
            LatLon::from_degrees(20.0, 15.0),
        ]),
    });
    w
}

#[test]
fn golden_world_hash() {
    let got = to_hex(&world_hash(&build()));
    assert_eq!(
        got, GOLDEN,
        "world hash changed; if intentional, set GOLDEN = \"{got}\""
    );
}

#[test]
fn world_hash_is_sensitive_to_content() {
    let a = build();
    let mut b = build();
    let id = b.registry().id_of("elevation").unwrap();
    let t = TileId::new(Face::NegX, 0, 0, 0).unwrap();
    let store = b.layer_mut::<f32>(id).unwrap();
    let v = store.get(t).unwrap().get(0, 0);
    store.get_mut(t).unwrap().set(0, 0, v + 1.0);
    assert_ne!(world_hash(&a), world_hash(&b));
}

#[test]
fn cache_keys_separate_every_input() {
    let src = SourceHash([7; 32]);
    let t = TileId::new(Face::PosY, 3, 1, 2).unwrap();
    let base = tile_cache_key(&src, 42, "0.1.0", "elevation", t);
    assert_eq!(base, tile_cache_key(&src, 42, "0.1.0", "elevation", t));
    assert_ne!(
        base,
        tile_cache_key(&SourceHash([8; 32]), 42, "0.1.0", "elevation", t)
    );
    assert_ne!(base, tile_cache_key(&src, 43, "0.1.0", "elevation", t));
    assert_ne!(base, tile_cache_key(&src, 42, "0.1.1", "elevation", t));
    assert_ne!(base, tile_cache_key(&src, 42, "0.1.0", "biome", t));
    assert_ne!(
        base,
        tile_cache_key(
            &src,
            42,
            "0.1.0",
            "elevation",
            TileId::new(Face::PosY, 3, 2, 1).unwrap()
        )
    );
    // Length-prefixing prevents "ab"+"c" colliding with "a"+"bc".
    assert_ne!(
        tile_cache_key(&src, 1, "ab", "c", t),
        tile_cache_key(&src, 1, "a", "bc", t)
    );
}
