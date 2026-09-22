use crate::tile::CellValue;
use crate::world::LayerStore;
use core::fmt;
use wb_grid::{TILE_SIZE, TileId};

pub const APRON_EDGE: u32 = TILE_SIZE + 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApronError {
    MissingTile(TileId),
}

impl fmt::Display for ApronError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApronError::MissingTile(t) => write!(f, "apron needs missing tile {t:?}"),
        }
    }
}

impl std::error::Error for ApronError {}

/// Index into a gathered apron buffer; `li, lj` range over `-1..=TILE_SIZE`.
pub fn apron_index(li: i32, lj: i32) -> usize {
    ((lj + 1) as u32 * APRON_EDGE + (li + 1) as u32) as usize
}

/// The tile's cells plus a one-cell ring copied from neighbouring tiles, so
/// stencil operations see identical values on both sides of every seam.
pub fn gather_with_apron<T: CellValue>(
    store: &LayerStore<T>,
    tile: TileId,
) -> Result<Vec<T>, ApronError> {
    let centre = store.get(tile).ok_or(ApronError::MissingTile(tile))?;
    let mut out = vec![T::default(); (APRON_EDGE * APRON_EDGE) as usize];
    let last = TILE_SIZE as i32 - 1;
    for lj in -1..=TILE_SIZE as i32 {
        for li in -1..=TILE_SIZE as i32 {
            let (ci, cj) = (li.clamp(0, last), lj.clamp(0, last));
            let value = if ci == li && cj == lj {
                centre.get(ci as u32, cj as u32)
            } else {
                let cell = tile.cell(ci as u32, cj as u32).offset(li - ci, lj - cj);
                let (nt, ni, nj) = cell.tile();
                store
                    .get(nt)
                    .ok_or(ApronError::MissingTile(nt))?
                    .get(ni, nj)
            };
            out[apron_index(li, lj)] = value;
        }
    }
    Ok(out)
}
