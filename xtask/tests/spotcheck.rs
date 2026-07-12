//! WP3 acceptance harness: 10-body Horizons position spot-check
//! (spec §9). Dormant until the online capture run drops real data into
//! `fixtures/spotcheck/` — then it becomes the CI gate, no code changes.
//!
//! Expected files:
//! - `fixtures/spotcheck/catalog.ron` — emitted by a real `--online` run.
//! - `fixtures/spotcheck/vectors.json` — captured Horizons VECTORS truth:
//!   `[{ "id": "...", "jd_tdb": f64, "position_km": [x,y,z], "tol_km": f64 }]`
//!   Frame: ecliptic-J2000, **parent-centric** (planets/small bodies about the
//!   Sun via CENTER='500@10'; moons about their parent), matching `state_at`
//!   output directly so no frame composition is needed here.
//!
//! Tolerances are per-category budgets, documented in the spec: two-body
//! motion vs full ephemeris — planets tightest, comets loosest (ignored
//! non-gravitational forces dominate).

use sim_core::catalog::Catalog;
use sim_core::kepler::state_at;
use sim_core::time::t_from_jd_tdb;
use std::path::PathBuf;

#[derive(serde::Deserialize)]
struct VectorFixture {
    id: String,
    jd_tdb: f64,
    position_km: [f64; 3],
    tol_km: f64,
}

#[test]
fn horizons_position_spot_check() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/spotcheck");
    let catalog_path = dir.join("catalog.ron");
    let vectors_path = dir.join("vectors.json");
    if !catalog_path.exists() || !vectors_path.exists() {
        eprintln!(
            "spot-check: no captures in {} — skipping (harness is armed; \
             drop catalog.ron + vectors.json from the online run to activate)",
            dir.display()
        );
        return;
    }

    let catalog =
        Catalog::from_ron_str(&std::fs::read_to_string(&catalog_path).unwrap()).unwrap();
    catalog.validate().expect("captured catalog must validate");
    let index = catalog.id_index();

    let vectors: Vec<VectorFixture> =
        serde_json::from_str(&std::fs::read_to_string(&vectors_path).unwrap()).unwrap();
    assert!(vectors.len() >= 10, "spec §9 calls for a 10-body spot-check set");

    let mut failures = Vec::new();
    for v in &vectors {
        let body = &catalog.bodies[*index.get(v.id.as_str()).expect("fixture id in catalog")];
        let orbit = body.orbit.as_ref().expect("spot-check body has an orbit");
        let parent = &catalog.bodies[*index
            .get(body.parent.as_deref().expect("spot-check body has a parent"))
            .unwrap()];
        let mu = parent.gm_km3_s2.expect("parent has GM");

        let s = state_at(orbit, mu, t_from_jd_tdb(v.jd_tdb)).unwrap();
        let d = [
            s.position_km[0] - v.position_km[0],
            s.position_km[1] - v.position_km[1],
            s.position_km[2] - v.position_km[2],
        ];
        let err = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if err > v.tol_km {
            failures.push(format!(
                "{} @ JD {}: {:.1} km off (budget {} km)",
                v.id, v.jd_tdb, err, v.tol_km
            ));
        } else {
            eprintln!("spot-check: {} @ JD {} within budget ({:.1} km)", v.id, v.jd_tdb, err);
        }
    }
    assert!(failures.is_empty(), "spot-check failures:\n{}", failures.join("\n"));
}
