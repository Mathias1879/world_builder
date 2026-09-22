use std::collections::BTreeSet;
use wb_grid::{CellId, Face, LatLon, TileId};
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

#[test]
fn same_meridian_box_is_zero_width() {
    assert_eq!(
        RegionBox::from_degrees(0.0, 5.0, 180.0, -180.0).unwrap_err(),
        ExtentError::ZeroWidth
    );
    // The full circle (-180 to 180) is still a valid box.
    assert!(RegionBox::from_degrees(0.0, 5.0, -180.0, 180.0).is_ok());
}

/// Every tile containing a point of a `nlon`×`nlat` lattice over the box
/// must be selected.
fn assert_covers_lattice(south: f64, north: f64, west: f64, east: f64, level: u8) {
    let r = RegionBox::from_degrees(south, north, west, east).unwrap();
    let selected: BTreeSet<TileId> = Extent::Region(r).tiles(level).into_iter().collect();
    let (nlon, nlat) = (400, 40);
    let mut missing = BTreeSet::new();
    for a in 0..nlon {
        for b in 0..nlat {
            let lon = west + (east - west) * a as f64 / (nlon - 1) as f64;
            let lat = south + (north - south) * b as f64 / (nlat - 1) as f64;
            let t = CellId::locate(LatLon::from_degrees(lat, lon).to_vec3(), level)
                .tile()
                .0;
            if !selected.contains(&t) {
                missing.insert(t);
            }
        }
    }
    assert!(
        missing.is_empty(),
        "{} tiles missing, e.g. {:?}",
        missing.len(),
        missing.first()
    );
}

#[test]
fn thin_band_selects_every_touched_tile() {
    assert_covers_lattice(10.0, 10.001, -40.0, 40.0, 8);
}

#[test]
fn narrow_band_selects_every_touched_tile() {
    assert_covers_lattice(10.0, 10.2, -40.0, 40.0, 8);
}

#[test]
fn thin_polar_rings_select_every_touched_tile() {
    assert_covers_lattice(80.0, 80.001, -180.0, 180.0, 6);
    assert_covers_lattice(-89.99, -89.9, -180.0, 180.0, 8);
}
