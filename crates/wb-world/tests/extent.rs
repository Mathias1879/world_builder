use wb_grid::{Face, LatLon, TileId};
use wb_world::{Extent, ExtentError, RegionBox};

#[test]
fn planet_covers_all_tiles() {
    assert_eq!(Extent::Planet.tiles(1), TileId::all(1));
    assert!(Extent::Planet.contains(LatLon::from_degrees(-80.0, 170.0)));
}

#[test]
fn validation() {
    assert_eq!(
        RegionBox::from_degrees(10.0, 5.0, 0.0, 1.0).unwrap_err(),
        ExtentError::LatitudeOrder
    );
    assert_eq!(
        RegionBox::from_degrees(-95.0, 5.0, 0.0, 1.0).unwrap_err(),
        ExtentError::LatitudeRange
    );
    assert_eq!(
        RegionBox::from_degrees(0.0, 5.0, 0.0, 190.0).unwrap_err(),
        ExtentError::LongitudeRange
    );
    assert_eq!(
        RegionBox::from_degrees(0.0, 5.0, 7.0, 7.0).unwrap_err(),
        ExtentError::ZeroWidth
    );
}

#[test]
fn antimeridian_box() {
    let r = RegionBox::from_degrees(-10.0, 10.0, 170.0, -170.0).unwrap();
    assert!(r.contains(LatLon::from_degrees(0.0, 180.0)));
    assert!(r.contains(LatLon::from_degrees(0.0, -175.0)));
    assert!(!r.contains(LatLon::from_degrees(0.0, 0.0)));
}

#[test]
fn polar_cap_region_tiles() {
    let r = Extent::Region(RegionBox::from_degrees(60.0, 70.0, -10.0, 10.0).unwrap());
    let f = Face::PosZ;
    assert_eq!(r.tiles(0), vec![TileId::new(f, 0, 0, 0).unwrap()]);
    assert_eq!(
        r.tiles(1),
        vec![
            TileId::new(f, 1, 0, 0).unwrap(),
            TileId::new(f, 1, 1, 0).unwrap()
        ]
    );
}

#[test]
fn northern_hemisphere_excludes_south_face() {
    let r = Extent::Region(RegionBox::from_degrees(0.0, 90.0, -180.0, 180.0).unwrap());
    let faces: Vec<Face> = r.tiles(0).iter().map(|t| t.face).collect();
    assert_eq!(
        faces,
        vec![Face::PosX, Face::NegX, Face::PosY, Face::NegY, Face::PosZ]
    );
}

#[test]
fn tiny_region_inside_one_tile_is_found() {
    let r = Extent::Region(RegionBox::from_degrees(12.0, 12.01, 33.0, 33.01).unwrap());
    let tiles = r.tiles(5);
    assert!(!tiles.is_empty() && tiles.len() <= 4, "{tiles:?}");
}
