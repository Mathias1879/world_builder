use core::f64::consts::PI;
use wb_grid::{CellId, Face, TileId, cells_per_edge};
use wb_world::{
    Dtype, ENGINE_VERSION, Extent, LayerDesc, LodPolicy, Planet, RegistryError, Tile, World,
    WorldError,
};

fn elevation() -> LayerDesc {
    LayerDesc {
        name: "elevation".into(),
        dtype: Dtype::F32,
        unit: "m".into(),
        producer: "test".into(),
        lod: LodPolicy::AreaMean,
    }
}

#[test]
fn engine_version_is_set() {
    assert!(!ENGINE_VERSION.is_empty());
}

#[test]
fn planet_cell_areas_sum_to_sphere_area() {
    let planet = Planet::default();
    assert_eq!(planet.radius_m, 6_371_000.0);
    let m = cells_per_edge(0);
    let face_area: f64 = (0..m)
        .flat_map(|j| (0..m).map(move |i| (i, j)))
        .map(|(i, j)| planet.cell_area_m2(CellId::new(Face::PosX, 0, i, j).unwrap()))
        .sum();
    let sphere = 4.0 * PI * planet.radius_m * planet.radius_m;
    assert!((face_area * 6.0 - sphere).abs() < 1e-8 * sphere);
}

#[test]
fn registry_rules() {
    let mut w = World::new(Planet::default(), Extent::Planet);
    let id = w.register_layer(elevation()).unwrap();
    assert_eq!(w.registry().id_of("elevation"), Some(id));
    assert_eq!(w.registry().get(id).unwrap().unit, "m");
    assert_eq!(
        w.register_layer(elevation()).unwrap_err(),
        WorldError::Registry(RegistryError::DuplicateName("elevation".into()))
    );
    let bad = LayerDesc {
        name: "biome".into(),
        dtype: Dtype::U8,
        lod: LodPolicy::AreaMean,
        ..elevation()
    };
    assert_eq!(
        w.register_layer(bad).unwrap_err(),
        WorldError::Registry(RegistryError::PolicyNotSupported {
            name: "biome".into(),
            dtype: Dtype::U8
        })
    );
}

#[test]
fn typed_layer_access() {
    let mut w = World::new(Planet::default(), Extent::Planet);
    let id = w.register_layer(elevation()).unwrap();
    let tid = TileId::new(Face::NegX, 0, 0, 0).unwrap();
    w.layer_mut::<f32>(id)
        .unwrap()
        .insert(Tile::from_fn(tid, |i, _| i as f32));
    let store = w.layer::<f32>(id).unwrap();
    assert_eq!(store.len(), 1);
    assert_eq!(store.cell(tid.cell(7, 3)), Some(7.0));
    assert_eq!(
        store.cell(TileId::new(Face::PosX, 0, 0, 0).unwrap().cell(0, 0)),
        None
    );
    assert_eq!(
        w.layer::<u8>(id).unwrap_err(),
        WorldError::DtypeMismatch {
            layer: Dtype::F32,
            requested: Dtype::U8
        }
    );
    assert_eq!(
        w.layer::<f32>(wb_world::LayerId(99)).unwrap_err(),
        WorldError::UnknownLayer(wb_world::LayerId(99))
    );
}
