use proptest::prelude::*;
use wb_grid::{CellId, Face, MAX_LEVEL, TILE_SIZE, TileId, cells_per_edge, tiles_per_edge};

#[test]
fn sizes() {
    assert_eq!(tiles_per_edge(0), 1);
    assert_eq!(tiles_per_edge(3), 8);
    assert_eq!(cells_per_edge(0), 256);
    assert_eq!(cells_per_edge(MAX_LEVEL), 256 * 4096);
}

#[test]
fn bounds_are_checked() {
    assert!(TileId::new(Face::PosX, 1, 1, 1).is_some());
    assert!(TileId::new(Face::PosX, 1, 2, 0).is_none());
    assert!(TileId::new(Face::PosX, MAX_LEVEL + 1, 0, 0).is_none());
    assert!(CellId::new(Face::NegZ, 0, 255, 255).is_some());
    assert!(CellId::new(Face::NegZ, 0, 256, 0).is_none());
}

#[test]
fn tile_cell_mapping() {
    let t = TileId::new(Face::PosY, 2, 3, 1).unwrap();
    let c = t.cell(10, 20);
    assert_eq!((c.i, c.j), (3 * TILE_SIZE + 10, TILE_SIZE + 20));
    assert_eq!(c.tile(), (t, 10, 20));
}

#[test]
fn tile_hierarchy() {
    let t = TileId::new(Face::NegY, 3, 5, 6).unwrap();
    let kids = t.children().unwrap();
    for k in kids {
        assert_eq!(k.parent(), Some(t));
    }
    assert_eq!(TileId::new(Face::NegY, 0, 0, 0).unwrap().parent(), None);
    assert_eq!(
        TileId::new(Face::NegY, MAX_LEVEL, 0, 0).unwrap().children(),
        None
    );
}

#[test]
fn all_tiles_sorted_and_counted() {
    let all = TileId::all(1);
    assert_eq!(all.len(), 24);
    let mut sorted = all.clone();
    sorted.sort();
    assert_eq!(all, sorted);
}

#[test]
#[should_panic(expected = "local cell index out of range")]
fn tile_cell_rejects_out_of_range_local_index() {
    TileId::new(Face::PosX, 0, 0, 0).unwrap().cell(TILE_SIZE, 0);
}

fn face_strategy() -> impl Strategy<Value = Face> {
    (0u8..6).prop_map(|i| Face::from_index(i).unwrap())
}

fn cell_strategy() -> impl Strategy<Value = CellId> {
    (face_strategy(), 0u8..=MAX_LEVEL).prop_flat_map(|(face, level)| {
        let m = cells_per_edge(level);
        (0..m, 0..m).prop_map(move |(i, j)| CellId::new(face, level, i, j).unwrap())
    })
}

fn cfg() -> ProptestConfig {
    ProptestConfig {
        cases: 512,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(cfg())]
    #[test]
    fn locate_inverts_center(c in cell_strategy()) {
        prop_assert_eq!(CellId::locate(c.center(), c.level), c);
    }

    #[test]
    fn cell_is_child_of_its_parent(c in cell_strategy()) {
        if let Some(p) = c.parent() {
            prop_assert!(p.children().unwrap().contains(&c));
        } else {
            prop_assert_eq!(c.level, 0);
        }
    }
}
