//! WP3 acceptance harness: 10-body Horizons position spot-check
//! (spec §9). Dormant until the online capture run drops real data into
//! `fixtures/spotcheck/` — then it becomes the CI gate, no code changes.
//!
//! Expected files:
//! - `fixtures/spotcheck/catalog.ron` — emitted by a real `--online` run.
//! - `fixtures/spotcheck/vectors.json` — captured Horizons VECTORS truth:
//!   `[{ "id": "...", "jd_tdb": f64, "position_km": [x,y,z], "tol_km": f64, "gate": bool }]`
//!   Frame: ecliptic-J2000, **parent-centric** (planets/small bodies about the
//!   Sun via CENTER='500@10'; moons about their parent), matching `state_at`
//!   output directly so no frame composition is needed here.
//!
//! Q6 retains both captured epochs for audit, but gates all 10 bodies at the
//! catalog epoch plus Halley at its 1986 perihelion/demo epoch. The other nine
//! historical points carry `gate: false`.
//!
//! Tolerances are per-category budgets, documented in the spec: two-body
//! motion vs full ephemeris — planets tightest, comets loosest (ignored
//! non-gravitational forces dominate).

use sim_core::catalog::{Catalog, Category};
use sim_core::kepler::state_at;
use sim_core::time::t_from_jd_tdb;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(serde::Deserialize)]
struct VectorFixture {
    id: String,
    jd_tdb: f64,
    position_km: [f64; 3],
    tol_km: f64,
    gate: bool,
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

    let catalog = Catalog::from_ron_str(&std::fs::read_to_string(&catalog_path).unwrap()).unwrap();
    catalog.validate().expect("captured catalog must validate");
    let index = catalog.id_index();

    let vectors: Vec<VectorFixture> =
        serde_json::from_str(&std::fs::read_to_string(&vectors_path).unwrap()).unwrap();
    assert_eq!(
        vectors.len(),
        20,
        "retain both captured epochs as audit data"
    );

    let gated: Vec<&VectorFixture> = vectors.iter().filter(|v| v.gate).collect();
    let gated_ids: HashSet<&str> = gated.iter().map(|v| v.id.as_str()).collect();
    assert_eq!(
        gated_ids.len(),
        10,
        "the active gate must exercise all 10 approved bodies"
    );
    for id in &gated_ids {
        assert!(
            gated.iter().any(|v| v.id == *id && v.jd_tdb == 2461042.0),
            "'{id}' must be gated at the catalog epoch"
        );
    }
    assert!(
        gated
            .iter()
            .any(|v| v.id == "halley" && v.jd_tdb == 2446471.0),
        "Halley's 1986 perihelion/demo epoch must remain gated"
    );
    assert!(
        gated
            .iter()
            .all(|v| v.jd_tdb != 2446471.0 || v.id == "halley"),
        "the other nine 1986 vectors are audit-only under the Q6 decision"
    );

    let mut failures = Vec::new();
    for v in gated {
        let body = &catalog.bodies[*index.get(v.id.as_str()).expect("fixture id in catalog")];
        let expected_tolerance = match body.category {
            Category::Planet => 1.0,
            Category::Moon => 10.0,
            Category::DwarfPlanet => 25_000.0,
            Category::Comet => 30_000_000.0,
            category => panic!("no approved spot-check budget for {category:?}"),
        };
        assert_eq!(
            v.tol_km, expected_tolerance,
            "{} must use its documented category budget",
            v.id
        );
        let orbit = body.orbit.as_ref().expect("spot-check body has an orbit");
        let parent = &catalog.bodies[*index
            .get(
                body.parent
                    .as_deref()
                    .expect("spot-check body has a parent"),
            )
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
            eprintln!(
                "spot-check: {} @ JD {} within budget ({:.1} km)",
                v.id, v.jd_tdb, err
            );
        }
    }
    assert!(
        failures.is_empty(),
        "spot-check failures:\n{}",
        failures.join("\n")
    );
}
