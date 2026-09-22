use core::fmt;
use wb_grid::{Face, TILE_SIZE, TileId};

pub const TILE_CELLS: usize = (TILE_SIZE * TILE_SIZE) as usize;

const MAGIC: &[u8; 4] = b"WBT1";
const HEADER: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Dtype {
    F32 = 0,
    U8 = 1,
    U16 = 2,
    I16 = 3,
}

/// A value storable in a tile, with a fixed little-endian encoding.
pub trait CellValue: Copy + Default + PartialEq + fmt::Debug + 'static {
    const DTYPE: Dtype;
    const BYTES: usize;
    fn write_le(self, out: &mut Vec<u8>);
    fn read_le(bytes: &[u8]) -> Self;
}

macro_rules! cell_value {
    ($t:ty, $dtype:expr, $n:expr) => {
        impl CellValue for $t {
            const DTYPE: Dtype = $dtype;
            const BYTES: usize = $n;
            fn write_le(self, out: &mut Vec<u8>) {
                out.extend_from_slice(&self.to_le_bytes());
            }
            fn read_le(bytes: &[u8]) -> Self {
                let mut a = [0u8; $n];
                a.copy_from_slice(&bytes[..$n]);
                <$t>::from_le_bytes(a)
            }
        }
    };
}

/// Canonical quiet NaN written for every f32 NaN, so the encoding (and hash)
/// does not depend on a target's NaN sign or payload.
const CANONICAL_NAN_F32: u32 = 0x7FC0_0000;

impl CellValue for f32 {
    const DTYPE: Dtype = Dtype::F32;
    const BYTES: usize = 4;
    fn write_le(self, out: &mut Vec<u8>) {
        let bits = if self.is_nan() {
            CANONICAL_NAN_F32
        } else {
            self.to_bits()
        };
        out.extend_from_slice(&bits.to_le_bytes());
    }
    fn read_le(bytes: &[u8]) -> Self {
        let mut a = [0u8; 4];
        a.copy_from_slice(&bytes[..4]);
        f32::from_le_bytes(a)
    }
}

cell_value!(u8, Dtype::U8, 1);
cell_value!(u16, Dtype::U16, 2);
cell_value!(i16, Dtype::I16, 2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    BadMagic,
    WrongDtype,
    BadHeader,
    BadLength,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            DecodeError::BadMagic => "not a WBT1 tile",
            DecodeError::WrongDtype => "tile dtype does not match the requested type",
            DecodeError::BadHeader => "tile header has an invalid face, level, or position",
            DecodeError::BadLength => "tile payload has the wrong length",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for DecodeError {}

/// 256×256 cells of one layer, row-major (`j * 256 + i`).
#[derive(Clone, Debug, PartialEq)]
pub struct Tile<T: CellValue> {
    id: TileId,
    data: Vec<T>,
}

impl<T: CellValue> Tile<T> {
    pub fn new(id: TileId) -> Self {
        Self {
            id,
            data: vec![T::default(); TILE_CELLS],
        }
    }

    pub fn from_fn(id: TileId, mut f: impl FnMut(u32, u32) -> T) -> Self {
        let mut data = Vec::with_capacity(TILE_CELLS);
        for j in 0..TILE_SIZE {
            for i in 0..TILE_SIZE {
                data.push(f(i, j));
            }
        }
        Self { id, data }
    }

    pub fn id(&self) -> TileId {
        self.id
    }

    pub fn get(&self, i: u32, j: u32) -> T {
        self.data[(j * TILE_SIZE + i) as usize]
    }

    pub fn set(&mut self, i: u32, j: u32, v: T) {
        self.data[(j * TILE_SIZE + i) as usize] = v;
    }

    pub fn data(&self) -> &[T] {
        &self.data
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER + TILE_CELLS * T::BYTES);
        out.extend_from_slice(MAGIC);
        out.push(T::DTYPE as u8);
        out.push(self.id.face.index());
        out.push(self.id.level);
        out.push(0);
        out.extend_from_slice(&self.id.x.to_le_bytes());
        out.extend_from_slice(&self.id.y.to_le_bytes());
        for v in &self.data {
            v.write_le(&mut out);
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < HEADER || &bytes[..4] != MAGIC {
            return Err(DecodeError::BadMagic);
        }
        if bytes[4] != T::DTYPE as u8 {
            return Err(DecodeError::WrongDtype);
        }
        let face = Face::from_index(bytes[5]).ok_or(DecodeError::BadHeader)?;
        let x = u32::from_le_bytes(bytes[8..12].try_into().expect("4 bytes"));
        let y = u32::from_le_bytes(bytes[12..16].try_into().expect("4 bytes"));
        let id = TileId::new(face, bytes[6], x, y).ok_or(DecodeError::BadHeader)?;
        if bytes.len() != HEADER + TILE_CELLS * T::BYTES {
            return Err(DecodeError::BadLength);
        }
        let data = bytes[HEADER..]
            .chunks_exact(T::BYTES)
            .map(T::read_le)
            .collect();
        Ok(Self { id, data })
    }

    pub fn content_hash(&self) -> [u8; 32] {
        blake3::hash(&self.encode()).into()
    }
}
