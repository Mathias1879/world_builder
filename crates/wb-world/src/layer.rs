use crate::tile::Dtype;
use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerId(pub u16);

/// How a parent cell's value is derived from its 4 children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LodPolicy {
    /// Area-weighted mean ("coarse is truth"); F32 only.
    AreaMean = 0,
    /// Most common value (categorical layers such as biome).
    Mode = 1,
    /// Maximum value.
    Max = 2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayerDesc {
    pub name: String,
    pub dtype: Dtype,
    pub unit: String,
    pub producer: String,
    pub lod: LodPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateName(String),
    PolicyNotSupported { name: String, dtype: Dtype },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::DuplicateName(n) => write!(f, "layer '{n}' is already registered"),
            RegistryError::PolicyNotSupported { name, dtype } => {
                write!(f, "layer '{name}': AreaMean requires F32, got {dtype:?}")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

#[derive(Clone, Debug, Default)]
pub struct LayerRegistry {
    descs: Vec<LayerDesc>,
}

impl LayerRegistry {
    pub fn register(&mut self, desc: LayerDesc) -> Result<LayerId, RegistryError> {
        if self.id_of(&desc.name).is_some() {
            return Err(RegistryError::DuplicateName(desc.name));
        }
        if desc.lod == LodPolicy::AreaMean && desc.dtype != Dtype::F32 {
            return Err(RegistryError::PolicyNotSupported {
                name: desc.name,
                dtype: desc.dtype,
            });
        }
        let id = LayerId(u16::try_from(self.descs.len()).expect("fewer than 65536 layers"));
        self.descs.push(desc);
        Ok(id)
    }

    pub fn get(&self, id: LayerId) -> Option<&LayerDesc> {
        self.descs.get(id.0 as usize)
    }

    pub fn id_of(&self, name: &str) -> Option<LayerId> {
        self.descs
            .iter()
            .position(|d| d.name == name)
            .map(|k| LayerId(k as u16))
    }

    pub fn iter(&self) -> impl Iterator<Item = (LayerId, &LayerDesc)> {
        self.descs
            .iter()
            .enumerate()
            .map(|(k, d)| (LayerId(k as u16), d))
    }
}
