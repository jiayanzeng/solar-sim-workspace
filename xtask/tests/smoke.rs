//! Offline end-to-end smoke: fixtures → fetch → parse → normalize → validate
//! → emit → reload through the sim-core loader. This is the same code path
//! the real (online) run takes; only the `Fetch` impl differs.

use sim_core::catalog::Catalog;
use std::path::PathBuf;
use xtask::{fetch::Fixtures, generate, GenOptions};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[test]
fn fixture_catalog_contents_are_correct() {
    let opts = GenOptions {
        allow_partial: true,
        ..Default::default()
    };
    let (catalog, skipped) = generate(
        &Fixtures {
            dir: fixtures_dir(),
        },
        &opts,
    )
    .unwrap();

    let ids: Vec<&str> = catalog.bodies.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["sun", "earth", "moon", "ceres", "halley", "3i_atlas"]
    );
    assert_eq!(skipped.len(), 60, "everything without a fixture is skipped");

    // Planet route: fitted mean motion from the two near-epoch records.
    let earth = catalog.find("Earth").unwrap();
    let n = earth
        .orbit
        .as_ref()
        .unwrap()
        .mean_motion_deg_per_day
        .unwrap();
    assert!((n - 0.9856).abs() < 1e-6, "fitted n = {n}");

    // Moon route: parent-centric single-epoch record, runtime n from parent GM.
    let moon = catalog.find("Luna").unwrap();
    let mo = moon.orbit.as_ref().unwrap();
    assert!(mo.mean_motion_deg_per_day.is_none());
    let gm_earth = earth.gm_km3_s2.unwrap();
    let period_days = mo.period_s(gm_earth).unwrap() / 86_400.0;
    assert!(
        (period_days - 27.3).abs() < 0.5,
        "sidereal-ish month, got {period_days}"
    );

    // SBDB comet with tp only: epoch re-based to perihelion.
    let halley = catalog.find("1P").unwrap();
    let h = halley.orbit.as_ref().unwrap();
    assert!((h.epoch_jd_tdb - 2446467.395).abs() < 1e-9);
    assert_eq!(h.elements.m0_deg, 0.0);
    assert!(!h.elements.is_hyperbolic());

    // Hyperbolic branch: negative a, e > 1.
    let atlas = catalog.find("C/2025 N1").unwrap();
    let a = atlas.orbit.as_ref().unwrap();
    assert!(a.elements.is_hyperbolic());
    assert!(a.elements.a_km < 0.0);
}

#[test]
fn emitted_ron_reloads_through_sim_core() {
    let opts = GenOptions {
        allow_partial: true,
        ..Default::default()
    };
    let (catalog, _) = generate(
        &Fixtures {
            dir: fixtures_dir(),
        },
        &opts,
    )
    .unwrap();

    let out = std::env::temp_dir().join("catalog.smoke.ron");
    xtask::emit::write_catalog(&catalog, &out, "smoke test").unwrap();

    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.starts_with("// GENERATED FILE"));
    let reloaded = Catalog::from_ron_str(&text).expect("app loader must accept emitted file");
    reloaded.validate().expect("emitted file must validate");
    assert_eq!(reloaded.bodies.len(), catalog.bodies.len());
}
