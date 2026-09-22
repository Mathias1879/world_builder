use wb_grid::{CellId, Face, TILE_SIZE, TileId};
use wb_world::{APRON_EDGE, ApronError, LayerStore, Tile, apron_index, gather_with_apron};

/// Level-0 store where each cell holds its own global index (exact in f32).
fn indexed_store() -> LayerStore<f32> {
    let mut s = LayerStore::default();
    for t in TileId::all(0) {
        let base = t.face.index() as u32 * 65_536;
        s.insert(Tile::from_fn(t, |i, j| (base + j * 256 + i) as f32));
    }
    s
}

fn decode(v: f32) -> CellId {
    let k = v as u32;
    CellId::new(
        Face::from_index((k / 65_536) as u8).unwrap(),
        0,
        k % 256,
        (k % 65_536) / 256,
    )
    .unwrap()
}

fn ring() -> impl Iterator<Item = (i32, i32)> {
    let n = TILE_SIZE as i32;
    (-1..=n)
        .flat_map(move |lj| (-1..=n).map(move |li| (li, lj)))
        .filter(move |(li, lj)| {
            let outside_i = *li < 0 || *li >= n;
            let outside_j = *lj < 0 || *lj >= n;
            outside_i != outside_j // edge ring, excluding the 4 corners
        })
}

#[test]
fn interior_is_the_tile_itself() {
    let s = indexed_store();
    let t = TileId::new(Face::PosY, 0, 0, 0).unwrap();
    let a = gather_with_apron(&s, t).unwrap();
    assert_eq!(a.len(), (APRON_EDGE * APRON_EDGE) as usize);
    assert_eq!(a[apron_index(12, 34)], s.get(t).unwrap().get(12, 34));
}

#[test]
fn apron_crosses_faces() {
    let s = indexed_store();
    let t = TileId::new(Face::PosX, 0, 0, 0).unwrap();
    let a = gather_with_apron(&s, t).unwrap();
    // Past the +u edge of PosX lies PosY.
    assert_eq!(
        decode(a[apron_index(TILE_SIZE as i32, 100)]).face,
        Face::PosY
    );
}

#[test]
fn seams_are_mutual_on_every_face() {
    let s = indexed_store();
    for t in TileId::all(0) {
        let a = gather_with_apron(&s, t).unwrap();
        let last = TILE_SIZE as i32 - 1;
        for (li, lj) in ring() {
            let own = t.cell(li.clamp(0, last) as u32, lj.clamp(0, last) as u32);
            let neighbour = decode(a[apron_index(li, lj)]);
            assert!(
                neighbour.neighbors4().contains(&own),
                "{t:?} ({li},{lj}): {neighbour:?} does not see {own:?}"
            );
        }
    }
}

#[test]
fn missing_neighbour_is_reported() {
    let mut s = LayerStore::<f32>::default();
    let t = TileId::new(Face::NegZ, 0, 0, 0).unwrap();
    s.insert(Tile::new(t));
    assert!(matches!(
        gather_with_apron(&s, t),
        Err(ApronError::MissingTile(_))
    ));
}
