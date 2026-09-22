use wb_grid::{Face, TileId};
use wb_world::{DecodeError, TILE_CELLS, Tile};

fn tid() -> TileId {
    TileId::new(Face::PosY, 2, 1, 3).unwrap()
}

#[test]
fn from_fn_is_row_major() {
    let t: Tile<u16> = Tile::from_fn(tid(), |i, j| (j * 256 + i) as u16);
    assert_eq!(t.data().len(), TILE_CELLS);
    assert_eq!(t.get(5, 0), 5);
    assert_eq!(t.get(0, 1), 256);
    assert_eq!(t.data()[256 * 7 + 9], t.get(9, 7));
}

#[test]
fn encode_decode_roundtrip() {
    let t: Tile<f32> = Tile::from_fn(tid(), |i, j| i as f32 * 0.5 - j as f32);
    let bytes = t.encode();
    assert_eq!(&bytes[..4], b"WBT1");
    assert_eq!(bytes.len(), 16 + TILE_CELLS * 4);
    assert_eq!(Tile::<f32>::decode(&bytes).unwrap(), t);
}

#[test]
fn decode_rejects_bad_input() {
    let t: Tile<i16> = Tile::new(tid());
    let bytes = t.encode();
    assert_eq!(
        Tile::<u16>::decode(&bytes).unwrap_err(),
        DecodeError::WrongDtype
    );
    assert_eq!(
        Tile::<i16>::decode(&bytes[..100]).unwrap_err(),
        DecodeError::BadLength
    );
    let mut bad = bytes.clone();
    bad[0] = b'X';
    assert_eq!(
        Tile::<i16>::decode(&bad).unwrap_err(),
        DecodeError::BadMagic
    );
}

#[test]
fn content_hash_tracks_content() {
    let mut t: Tile<u8> = Tile::new(tid());
    let h0 = t.content_hash();
    assert_eq!(h0, Tile::<u8>::new(tid()).content_hash());
    t.set(3, 4, 9);
    assert_ne!(t.content_hash(), h0);
}

#[test]
fn nan_bit_patterns_encode_identically() {
    let nans = [f32::NAN, -f32::NAN, f32::from_bits(0x7FC0_1234)];
    let tiles: Vec<Tile<f32>> = nans
        .iter()
        .map(|&nan| Tile::from_fn(tid(), |i, j| if (i + j) % 7 == 0 { nan } else { i as f32 }))
        .collect();
    let bytes = tiles[0].encode();
    for t in &tiles[1..] {
        assert_eq!(t.encode(), bytes);
        assert_eq!(t.content_hash(), tiles[0].content_hash());
    }
    let decoded = Tile::<f32>::decode(&bytes).unwrap();
    assert!(decoded.get(0, 0).is_nan());
    assert_eq!(decoded.get(1, 0), 1.0);
}
