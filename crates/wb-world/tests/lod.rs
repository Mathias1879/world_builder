use proptest::prelude::*;
use wb_grid::{CellId, Face, TILE_SIZE, TileId};
use wb_world::{
    Aggregate, LayerStore, LodError, LodPolicy, Tile, downsample, refine_cell, refine_tile,
};

const TOL_M: f32 = 2e-3;

fn cfg() -> ProptestConfig {
    ProptestConfig {
        cases: 256,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(cfg())]
    #[test]
    fn refine_preserves_area_weighted_mean(
        parent in -11_000.0f32..9_000.0,
        detail in prop::array::uniform4(-2_000.0f32..2_000.0),
        face in 0u8..6, i in 0u32..512, j in 0u32..512,
    ) {
        let p = CellId::new(Face::from_index(face).unwrap(), 1, i, j).unwrap();
        let w = p.children().unwrap().map(|c| c.area_unit());
        let kids = refine_cell(parent, detail, w);
        let mean = f32::area_mean(kids, w).unwrap();
        prop_assert!((mean - parent).abs() <= TOL_M, "mean {mean} vs parent {parent}");
    }
}

fn detail(c: CellId) -> f32 {
    let (s, t) = c.center_st();
    (300.0 * libm::sin(97.0 * s) * libm::cos(61.0 * t)) as f32
}

#[test]
fn refine_then_downsample_returns_the_parent_tile() {
    let parent_id = TileId::new(Face::PosZ, 0, 0, 0).unwrap();
    let mut parents = LayerStore::<f32>::default();
    parents.insert(Tile::from_fn(parent_id, |i, j| {
        i as f32 * 20.0 - j as f32 * 13.0
    }));
    let mut kids = LayerStore::<f32>::default();
    for c in parent_id.children().unwrap() {
        kids.insert(refine_tile(&parents, c, detail).unwrap());
    }
    let back = downsample(&kids, parent_id, LodPolicy::AreaMean).unwrap();
    let orig = parents.get(parent_id).unwrap();
    for j in 0..TILE_SIZE {
        for i in 0..TILE_SIZE {
            assert!(
                (back.get(i, j) - orig.get(i, j)).abs() <= TOL_M,
                "cell ({i},{j})"
            );
        }
    }
}

#[test]
fn mode_and_max_policies() {
    let parent = TileId::new(Face::NegY, 0, 0, 0).unwrap();
    let mut kids = LayerStore::<u8>::default();
    for c in parent.children().unwrap() {
        // Within each 2×2 block: three cells of 7 and one of 200.
        kids.insert(Tile::from_fn(c, |i, j| {
            if i % 2 == 1 && j % 2 == 1 { 200 } else { 7 }
        }));
    }
    let mode = downsample(&kids, parent, LodPolicy::Mode).unwrap();
    let max = downsample(&kids, parent, LodPolicy::Max).unwrap();
    assert!(mode.data().iter().all(|v| *v == 7));
    assert!(max.data().iter().all(|v| *v == 200));
    assert_eq!(
        downsample(&kids, parent, LodPolicy::AreaMean).unwrap_err(),
        LodError::PolicyNotSupported
    );
}

#[test]
fn missing_children_are_reported() {
    let parent = TileId::new(Face::PosX, 0, 0, 0).unwrap();
    let store = LayerStore::<f32>::default();
    let first_child = parent.children().unwrap()[0];
    assert_eq!(
        downsample(&store, parent, LodPolicy::AreaMean).unwrap_err(),
        LodError::MissingTile(first_child)
    );
    assert_eq!(
        refine_tile(&store, parent, detail).unwrap_err(),
        LodError::NoParentLevel
    );
}
