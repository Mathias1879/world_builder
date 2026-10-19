//! Cross-target determinism gate. Every number `SkyModel` computes for a fixed system is
//! hashed bit-for-bit; the same hash must come out on aarch64, wasm32-wasip1 and
//! x86_64-musl (`scripts/ci-local.sh` runs all three). Update GOLDEN_SKY in the same
//! commit as any intentional change to the physics.

mod common;

use common::aethoria;
use wb_astro::SkyModel;
use wb_grid::LatLon;
use wb_world::to_hex;

const GOLDEN_SKY: &str = "a854cd7eeacb6073cbfd281eb70bd19991a133d3f957462a2c8c2c2ca7065051";

#[test]
fn golden_sky() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let mut h = blake3::Hasher::new();
    h.update(b"wb-astro-golden-v1");
    // A free function rather than a closure over `h`: a closure capturing `h` by unique
    // reference would hold that borrow for as long as the closure itself is live, which
    // conflicts with the bare `h.update(...)` calls interleaved between its uses below.
    fn bits(h: &mut blake3::Hasher, x: f64) {
        h.update(&x.to_bits().to_le_bytes());
    }

    // Derived values for every body, in EntityId order.
    for b in sky.bodies() {
        h.update(b.name.as_bytes());
        bits(&mut h, b.a_m);
        bits(&mut h, b.period_s);
        bits(&mut h, b.mass_kg);
        bits(&mut h, b.radius_m);
        bits(&mut h, b.density_kgm3);
        bits(&mut h, b.albedo);
        bits(&mut h, b.luminosity_w.unwrap_or(0.0));
        h.update(&b.order.to_le_bytes());
    }
    // Outputs through the year, at a stride that is coprime with every moon cycle.
    for k in 0..30 {
        let day = k as f64 * 13.0;
        bits(&mut h, sky.tide_range_m(day));
        for lat_deg in [-85.0, -45.0, 0.0, 23.5, 45.0, 85.0] {
            let lat = LatLon::from_degrees(lat_deg, 0.0);
            bits(&mut h, sky.insolation(lat, day));
            bits(&mut h, sky.daylight_hours(lat, day));
        }
        for b in sky.bodies() {
            if let Some(p) = sky.position(b.id, day) {
                bits(&mut h, p.x);
                bits(&mut h, p.y);
                bits(&mut h, p.z);
            }
            bits(&mut h, sky.phase(b.id, day).unwrap_or(-1.0));
            bits(&mut h, sky.apparent_brightness(b.id, day).unwrap_or(0.0));
        }
    }
    // Calendar arithmetic, both sides of the epoch.
    for d in [-390_001i64, -1, 0, 1, 389, 390, 1_000_000] {
        let date = sky.date_of(d);
        h.update(&date.year.to_le_bytes());
        h.update(&date.month_index.to_le_bytes());
        h.update(&date.day.to_le_bytes());
        h.update(&(date.weekday_index as u64).to_le_bytes());
    }
    // And the proposed seasons, which read the whole sunlight curve.
    for s in sky.propose_seasons(4) {
        h.update(s.code.as_str().as_bytes());
        for e in &s.creates {
            h.update(e.kind.as_bytes());
            for (f, v) in &e.fields {
                h.update(f.as_bytes());
                value_bytes(&mut h, v);
            }
        }
    }
    let got = to_hex(h.finalize().as_bytes());
    assert_eq!(
        got, GOLDEN_SKY,
        "sky values changed; if intentional, set GOLDEN_SKY = \"{got}\""
    );
}

fn value_bytes(h: &mut blake3::Hasher, v: &wb_editlog::Value) {
    match v {
        wb_editlog::Value::Text(s) => {
            h.update(b"t");
            h.update(s.as_bytes());
        }
        wb_editlog::Value::Int(n) => {
            h.update(b"i");
            h.update(&n.to_le_bytes());
        }
        wb_editlog::Value::Entity(e) => {
            h.update(b"e");
            h.update(&e.op.lamport.to_le_bytes());
            h.update(&e.n.to_le_bytes());
        }
        wb_editlog::Value::Float(x) => {
            h.update(b"f");
            h.update(&x.to_bits().to_le_bytes());
        }
        other => {
            // Not a stable wire format — no `Value` variant a `creates` entity actually
            // uses should ever reach here; if one does, the golden moves and says so.
            h.update(b"?");
            h.update(format!("{other:?}").as_bytes());
        }
    }
}
