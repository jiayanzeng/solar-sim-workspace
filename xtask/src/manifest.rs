//! The curated half of the catalog (Rev B §4.1/§4.2).
//!
//! Split of responsibility, deliberately:
//! - **Curated here, human-reviewed:** identity (id/name/designation/aliases),
//!   taxonomy (category/parent), physical radius, GM for parents, display
//!   color, description blurbs, and the source route.
//! - **Generated from JPL, never hand-typed:** every orbital element, epoch,
//!   secular rate, and mean motion.
//!
//! REVIEW FLAGS: radii and the three TNO GM values below are curated from
//! standard references and MUST be confirmed against JPL physical-data pages
//! during the WP3 review pass (Rev B §4.2). Items marked `TODO(review)`.

use sim_core::catalog::Category;

/// IAU/JPL gravitational parameters, km³/s². TODO(review): verify against the
/// JPL DE440 constants file before catalog sign-off.
pub const GM_SUN: f64 = 1.327_124_400_18e11;
pub const GM_MERCURY: f64 = 2.203_186_8e4;
pub const GM_VENUS: f64 = 3.248_585_92e5;
pub const GM_EARTH: f64 = 3.986_004_418e5;
pub const GM_MARS: f64 = 4.282_837_5e4;
pub const GM_JUPITER: f64 = 1.266_865_32e8;
pub const GM_SATURN: f64 = 3.793_118_7e7;
pub const GM_URANUS: f64 = 5.793_939e6;
pub const GM_NEPTUNE: f64 = 6.836_529e6;
// TNO parents (needed because they carry moons). TODO(review): literature values.
pub const GM_PLUTO: f64 = 8.696e2;
pub const GM_ERIS: f64 = 1.108e3;
pub const GM_HAUMEA: f64 = 2.67e2;

// Category color LUT (per Rev B §9: planets individually colored; other
// categories share a hue). Our palette, not NASA's.
const C_SUN: (u8, u8, u8) = (255, 214, 140);
const C_MERCURY: (u8, u8, u8) = (158, 158, 158);
const C_VENUS: (u8, u8, u8) = (222, 184, 135);
const C_EARTH: (u8, u8, u8) = (86, 141, 235);
const C_MARS: (u8, u8, u8) = (204, 101, 66);
const C_JUPITER: (u8, u8, u8) = (211, 177, 140);
const C_SATURN: (u8, u8, u8) = (226, 205, 159);
const C_URANUS: (u8, u8, u8) = (148, 207, 216);
const C_NEPTUNE: (u8, u8, u8) = (99, 125, 222);
const C_DWARF: (u8, u8, u8) = (186, 156, 255);
const C_AST: (u8, u8, u8) = (158, 163, 170);
const C_COMET: (u8, u8, u8) = (166, 216, 232);
const C_MOON: (u8, u8, u8) = (198, 189, 175);

/// How the generator obtains orbital elements for a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// No orbit; physical constants only (the Sun).
    SunFixed,
    /// JPL Horizons ELEMENTS, heliocentric (`CENTER='500@10'`), sampled at the
    /// catalog epoch, epoch+1d (mean-motion fit) and 1800–2300 coarse epochs
    /// (secular fit).
    HorizonsPlanet { command: &'static str },
    /// JPL Horizons ELEMENTS, parent-centric (`CENTER='500@<parent>'`),
    /// single sample at the catalog epoch.
    HorizonsMoon {
        command: &'static str,
        center: &'static str,
    },
    /// Same as `HorizonsMoon`, but the numeric COMMAND (and possibly the
    /// center designator) must first be resolved through the Horizons lookup
    /// API — the TNO satellites have no stable well-known codes.
    /// See spec "Open items". In `--fixtures` mode a plain `<id>.json`
    /// Horizons response is accepted directly.
    HorizonsLookupMoon {
        sstr: &'static str,
        parent_sstr: &'static str,
    },
    /// JPL Small-Body Database (`sbdb.api?sstr=...&full-prec=true`),
    /// heliocentric ecliptic-J2000 elements at the SBDB epoch. Comets without
    /// a mean anomaly are re-based to perihelion (epoch := Tp, M0 := 0).
    Sbdb { sstr: &'static str },
}

pub struct Entry {
    pub id: &'static str,
    pub name: &'static str,
    pub designation: Option<&'static str>,
    pub aliases: &'static [&'static str],
    pub category: Category,
    pub parent: Option<&'static str>,
    pub gm_km3_s2: Option<f64>,
    /// TODO(review): confirm every radius against JPL physical data.
    pub radius_km: f64,
    pub color: (u8, u8, u8),
    pub route: Route,
    /// Curated blurb (Info tab). Empty entries are WP10 content debt (lint).
    pub blurb: &'static str,
    /// Provenance note carried into the emitted `source` field.
    pub source_note: &'static str,
}

const PHYS_NOTE: &str = "phys: curated (IAU/JPL references, review pending)";

macro_rules! planet {
    ($id:literal, $name:literal, $cmd:literal, $gm:expr, $r:expr, $col:expr, $blurb:literal) => {
        Entry {
            id: $id,
            name: $name,
            designation: None,
            aliases: &[],
            category: Category::Planet,
            parent: Some("sun"),
            gm_km3_s2: Some($gm),
            radius_km: $r,
            color: $col,
            route: Route::HorizonsPlanet { command: $cmd },
            blurb: $blurb,
            source_note: "orbit: JPL Horizons ELEMENTS heliocentric ECLIPJ2000 (+fitted secular)",
        }
    };
}

macro_rules! moon {
    ($id:literal, $name:literal, $cmd:literal, $center:literal, $parent:literal, $r:expr) => {
        Entry {
            id: $id,
            name: $name,
            designation: None,
            aliases: &[],
            category: Category::Moon,
            parent: Some($parent),
            gm_km3_s2: None,
            radius_km: $r,
            color: C_MOON,
            route: Route::HorizonsMoon {
                command: $cmd,
                center: $center,
            },
            blurb: "",
            source_note: "orbit: JPL Horizons ELEMENTS parent-centric ECLIPJ2000",
        }
    };
}

macro_rules! sbdb {
    ($id:literal, $name:literal, $des:expr, $aliases:expr, $cat:expr, $gm:expr, $r:expr, $col:expr, $sstr:literal, $blurb:literal) => {
        Entry {
            id: $id,
            name: $name,
            designation: $des,
            aliases: $aliases,
            category: $cat,
            parent: Some("sun"),
            gm_km3_s2: $gm,
            radius_km: $r,
            color: $col,
            route: Route::Sbdb { sstr: $sstr },
            blurb: $blurb,
            source_note: "orbit: JPL SBDB heliocentric ECLIPJ2000",
        }
    };
}

/// The 66-body Rev B catalog, in emit order (parents before children).
pub fn entries() -> Vec<Entry> {
    use Category::*;
    let mut v: Vec<Entry> = Vec::with_capacity(66);

    // --- Star (1) ---
    v.push(Entry {
        id: "sun",
        name: "Sun",
        designation: None,
        aliases: &["Sol"],
        category: Star,
        parent: None,
        gm_km3_s2: Some(GM_SUN),
        radius_km: 695_700.0,
        color: C_SUN,
        route: Route::SunFixed,
        blurb: "The star at the center of the solar system, holding 99.8% of its mass. Every orbit in this catalog ultimately answers to it.",
        source_note: "no orbit (heliocentric anchor)",
    });

    // --- Planets (8) ---
    v.push(planet!(
        "mercury",
        "Mercury",
        "199",
        GM_MERCURY,
        2439.7,
        C_MERCURY,
        "The smallest planet and the closest to the Sun, racing through an 88-day year."
    ));
    v.push(planet!(
        "venus",
        "Venus",
        "299",
        GM_VENUS,
        6051.8,
        C_VENUS,
        "Earth's near-twin in size, wrapped in a crushing greenhouse atmosphere."
    ));
    v.push(planet!(
        "earth",
        "Earth",
        "399",
        GM_EARTH,
        6371.0,
        C_EARTH,
        "Our home planet — the only world known to harbor life."
    ));
    v.push(planet!(
        "mars",
        "Mars",
        "499",
        GM_MARS,
        3389.5,
        C_MARS,
        "The rusty desert world, home to the solar system's tallest volcano."
    ));
    v.push(planet!(
        "jupiter",
        "Jupiter",
        "5",
        GM_JUPITER,
        69_911.0,
        C_JUPITER,
        "The giant of the solar system — more massive than every other planet combined."
    ));
    v.push(planet!(
        "saturn",
        "Saturn",
        "6",
        GM_SATURN,
        58_232.0,
        C_SATURN,
        "The ringed gas giant, attended by a spectacular family of moons."
    ));
    v.push(planet!(
        "uranus",
        "Uranus",
        "7",
        GM_URANUS,
        25_362.0,
        C_URANUS,
        "An ice giant tipped on its side, orbiting the Sun once every 84 years."
    ));
    v.push(planet!(
        "neptune",
        "Neptune",
        "8",
        GM_NEPTUNE,
        24_622.0,
        C_NEPTUNE,
        "The outermost planet, a deep-blue ice giant with supersonic winds."
    ));

    // --- Moons (32) ---
    // Earth
    v.push({
        let mut m = moon!("moon", "Moon", "301", "500@399", "earth", 1737.4);
        m.aliases = &["Luna"];
        m.blurb = "Earth's constant companion and the only other world humans have walked on.";
        m
    });
    // Mars
    v.push(moon!("phobos", "Phobos", "401", "500@499", "mars", 11.1));
    v.push(moon!("deimos", "Deimos", "402", "500@499", "mars", 6.2));
    // Jupiter
    v.push(moon!("io", "Io", "501", "500@599", "jupiter", 1821.6));
    v.push(moon!(
        "europa", "Europa", "502", "500@599", "jupiter", 1560.8
    ));
    v.push(moon!(
        "ganymede", "Ganymede", "503", "500@599", "jupiter", 2634.1
    ));
    v.push(moon!(
        "callisto", "Callisto", "504", "500@599", "jupiter", 2410.3
    ));
    v.push(moon!(
        "amalthea", "Amalthea", "505", "500@599", "jupiter", 83.5
    ));
    v.push(moon!(
        "himalia", "Himalia", "506", "500@599", "jupiter", 75.0
    ));
    // Saturn
    v.push(moon!("mimas", "Mimas", "601", "500@699", "saturn", 198.2));
    v.push(moon!(
        "enceladus",
        "Enceladus",
        "602",
        "500@699",
        "saturn",
        252.1
    ));
    v.push(moon!("tethys", "Tethys", "603", "500@699", "saturn", 531.1));
    v.push(moon!("dione", "Dione", "604", "500@699", "saturn", 561.4));
    v.push(moon!("rhea", "Rhea", "605", "500@699", "saturn", 763.8));
    v.push(moon!("titan", "Titan", "606", "500@699", "saturn", 2574.7));
    v.push(moon!(
        "hyperion", "Hyperion", "607", "500@699", "saturn", 135.0
    ));
    v.push(moon!(
        "iapetus", "Iapetus", "608", "500@699", "saturn", 734.5
    ));
    v.push({
        let mut m = moon!("phoebe", "Phoebe", "609", "500@699", "saturn", 106.5);
        m.blurb = "A captured outer moon on a retrograde path (i ≈ 175°) — one of the catalog's two retrograde stress tests.";
        m
    });
    // Uranus
    v.push(moon!(
        "miranda", "Miranda", "705", "500@799", "uranus", 235.8
    ));
    v.push(moon!("ariel", "Ariel", "701", "500@799", "uranus", 578.9));
    v.push(moon!(
        "umbriel", "Umbriel", "702", "500@799", "uranus", 584.7
    ));
    v.push(moon!(
        "titania", "Titania", "703", "500@799", "uranus", 788.4
    ));
    v.push(moon!("oberon", "Oberon", "704", "500@799", "uranus", 761.4));
    // Neptune
    v.push({
        let mut m = moon!("triton", "Triton", "801", "500@899", "neptune", 1353.4);
        m.blurb = "Neptune's giant moon, orbiting backwards (i ≈ 157°) — almost certainly a captured Kuiper Belt object.";
        m
    });
    v.push({
        let mut m = moon!("nereid", "Nereid", "802", "500@899", "neptune", 170.0);
        m.blurb = "One of the most eccentric moon orbits known (e ≈ 0.75) — the catalog's high-eccentricity ellipse stress test.";
        m
    });
    v.push(moon!(
        "proteus", "Proteus", "808", "500@899", "neptune", 210.0
    ));

    // --- Dwarf planets (9) — before their moons; parents must precede children ---
    v.push(sbdb!("ceres", "Ceres", None, &["1 Ceres"], DwarfPlanet, None, 469.7, C_DWARF, "Ceres",
        "The largest object in the asteroid belt and the only dwarf planet of the inner solar system."));
    v.push(sbdb!(
        "pluto",
        "Pluto",
        None,
        &["134340"],
        DwarfPlanet,
        Some(GM_PLUTO),
        1188.3,
        C_DWARF,
        "134340",
        "The best-loved dwarf planet, ruling a five-moon system in the Kuiper Belt."
    ));
    v.push(sbdb!(
        "eris",
        "Eris",
        None,
        &[],
        DwarfPlanet,
        Some(GM_ERIS),
        1163.0,
        C_DWARF,
        "Eris",
        "The scattered-disc heavyweight whose discovery forced the definition of 'planet'."
    ));
    v.push(sbdb!(
        "haumea",
        "Haumea",
        None,
        &[],
        DwarfPlanet,
        Some(GM_HAUMEA),
        780.0,
        C_DWARF,
        "Haumea",
        "A fast-spinning, egg-shaped Kuiper Belt dwarf with two moons and a ring."
    ));
    v.push(sbdb!(
        "makemake",
        "Makemake",
        None,
        &[],
        DwarfPlanet,
        None,
        715.0,
        C_DWARF,
        "Makemake",
        ""
    ));
    v.push(sbdb!(
        "gonggong",
        "Gonggong",
        None,
        &[],
        DwarfPlanet,
        None,
        615.0,
        C_DWARF,
        "Gonggong",
        ""
    ));
    v.push(sbdb!(
        "quaoar",
        "Quaoar",
        None,
        &[],
        DwarfPlanet,
        None,
        545.0,
        C_DWARF,
        "Quaoar",
        ""
    ));
    v.push(sbdb!(
        "orcus",
        "Orcus",
        None,
        &[],
        DwarfPlanet,
        None,
        458.0,
        C_DWARF,
        "Orcus",
        ""
    ));
    v.push(sbdb!("sedna", "Sedna", None, &[], DwarfPlanet, None, 500.0, C_DWARF, "Sedna",
        "A distant world whose ~937 AU aphelion makes the sheer scale of the outer solar system visceral — the catalog's designated 'vastness' shot."));

    // --- TNO moons (belong to the 32-moon count) ---
    // Pluto
    v.push(moon!("charon", "Charon", "901", "500@999", "pluto", 606.0));
    v.push(moon!("nix", "Nix", "902", "500@999", "pluto", 19.5));
    v.push(moon!("hydra", "Hydra", "903", "500@999", "pluto", 18.0));
    // Eris / Haumea (Horizons ids resolved at generation time — see spec Open items)
    v.push(Entry {
        id: "dysnomia",
        name: "Dysnomia",
        designation: None,
        aliases: &[],
        category: Moon,
        parent: Some("eris"),
        gm_km3_s2: None,
        radius_km: 350.0,
        color: C_MOON,
        route: Route::HorizonsLookupMoon {
            sstr: "Dysnomia",
            parent_sstr: "Eris",
        },
        blurb: "",
        source_note: "orbit: JPL Horizons ELEMENTS parent-centric ECLIPJ2000 (id via lookup)",
    });
    v.push(Entry {
        id: "hiiaka",
        name: "Hi\u{02bb}iaka",
        designation: None,
        aliases: &["Hiiaka"],
        category: Moon,
        parent: Some("haumea"),
        gm_km3_s2: None,
        radius_km: 160.0,
        color: C_MOON,
        route: Route::HorizonsLookupMoon {
            sstr: "Hiiaka",
            parent_sstr: "Haumea",
        },
        blurb: "",
        source_note: "orbit: JPL Horizons ELEMENTS parent-centric ECLIPJ2000 (id via lookup)",
    });
    v.push(Entry {
        id: "namaka",
        name: "Namaka",
        designation: None,
        aliases: &[],
        category: Moon,
        parent: Some("haumea"),
        gm_km3_s2: None,
        radius_km: 85.0,
        color: C_MOON,
        route: Route::HorizonsLookupMoon {
            sstr: "Namaka",
            parent_sstr: "Haumea",
        },
        blurb: "",
        source_note: "orbit: JPL Horizons ELEMENTS parent-centric ECLIPJ2000 (id via lookup)",
    });

    // --- Asteroids (8) ---
    v.push(sbdb!(
        "pallas",
        "2 Pallas",
        None,
        &["Pallas"],
        Asteroid,
        None,
        256.0,
        C_AST,
        "2 Pallas",
        ""
    ));
    v.push(sbdb!(
        "juno",
        "3 Juno",
        None,
        &["Juno"],
        Asteroid,
        None,
        123.0,
        C_AST,
        "3 Juno",
        ""
    ));
    v.push(sbdb!(
        "vesta",
        "4 Vesta",
        None,
        &["Vesta"],
        Asteroid,
        None,
        262.7,
        C_AST,
        "4 Vesta",
        ""
    ));
    v.push(sbdb!(
        "hygiea",
        "10 Hygiea",
        None,
        &["Hygiea"],
        Asteroid,
        None,
        217.0,
        C_AST,
        "10 Hygiea",
        ""
    ));
    v.push(sbdb!(
        "psyche",
        "16 Psyche",
        None,
        &["Psyche"],
        Asteroid,
        None,
        113.0,
        C_AST,
        "16 Psyche",
        "A metal-rich world thought to be the exposed core of a shattered protoplanet."
    ));
    v.push(sbdb!(
        "eros",
        "433 Eros",
        None,
        &["Eros"],
        Asteroid,
        None,
        8.4,
        C_AST,
        "433 Eros",
        ""
    ));
    v.push(sbdb!(
        "bennu",
        "101955 Bennu",
        None,
        &["Bennu"],
        Asteroid,
        None,
        0.245,
        C_AST,
        "101955 Bennu",
        ""
    ));
    v.push(sbdb!(
        "apophis",
        "99942 Apophis",
        None,
        &["Apophis"],
        Asteroid,
        None,
        0.17,
        C_AST,
        "99942 Apophis",
        ""
    ));

    // --- Comets (8) ---
    v.push(sbdb!("halley", "1P/Halley", Some("1P"), &["Halley"], Comet, None, 5.5, C_COMET, "1P",
        "The most famous comet of all, returning roughly every 76 years; its 1986 perihelion is a demo-script stop."));
    v.push(sbdb!(
        "encke",
        "2P/Encke",
        Some("2P"),
        &["Encke"],
        Comet,
        None,
        2.4,
        C_COMET,
        "2P",
        ""
    ));
    v.push(sbdb!(
        "tempel_1",
        "9P/Tempel 1",
        Some("9P"),
        &["Tempel 1"],
        Comet,
        None,
        3.0,
        C_COMET,
        "9P",
        ""
    ));
    v.push(sbdb!(
        "churyumov_gerasimenko",
        "67P/Churyumov-Gerasimenko",
        Some("67P"),
        &["Churyumov-Gerasimenko"],
        Comet,
        None,
        2.0,
        C_COMET,
        "67P",
        ""
    ));
    v.push(sbdb!(
        "hartley_2",
        "103P/Hartley 2",
        Some("103P"),
        &["Hartley 2"],
        Comet,
        None,
        0.6,
        C_COMET,
        "103P",
        ""
    ));
    v.push(sbdb!(
        "hale_bopp",
        "Hale-Bopp",
        Some("C/1995 O1"),
        &[],
        Comet,
        None,
        30.0,
        C_COMET,
        "C/1995 O1",
        ""
    ));
    v.push(sbdb!(
        "neowise",
        "NEOWISE",
        Some("C/2020 F3"),
        &[],
        Comet,
        None,
        2.5,
        C_COMET,
        "C/2020 F3",
        ""
    ));
    v.push(sbdb!("3i_atlas", "3I/ATLAS", Some("C/2025 N1"), &["3I"], Comet, None, 2.5, C_COMET, "C/2025 N1",
        "The third known interstellar object, crossing the solar system on a hyperbolic path — the catalog's open-arc stress test."));

    v
}

/// Full source string for the emitted record.
pub fn source_string(e: &Entry) -> String {
    format!("{}; {}", e.source_note, PHYS_NOTE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn manifest_has_66_bodies_with_rev_b_category_counts() {
        let es = entries();
        assert_eq!(es.len(), 66);
        let count = |c: Category| es.iter().filter(|e| e.category == c).count();
        assert_eq!(count(Category::Star), 1);
        assert_eq!(count(Category::Planet), 8);
        assert_eq!(count(Category::DwarfPlanet), 9);
        assert_eq!(count(Category::Asteroid), 8);
        assert_eq!(count(Category::Moon), 32);
        assert_eq!(count(Category::Comet), 8);
    }

    #[test]
    fn manifest_ids_unique_and_parents_precede_children() {
        let es = entries();
        let mut seen: HashSet<&str> = HashSet::new();
        for e in &es {
            assert!(seen.insert(e.id), "duplicate id {}", e.id);
            if let Some(p) = e.parent {
                assert!(
                    seen.contains(p),
                    "parent '{}' of '{}' must precede it",
                    p,
                    e.id
                );
            }
        }
    }

    #[test]
    fn every_parent_has_gm() {
        let es = entries();
        let parents: HashSet<&str> = es.iter().filter_map(|e| e.parent).collect();
        for e in &es {
            if parents.contains(e.id) {
                assert!(e.gm_km3_s2.is_some(), "parent '{}' missing GM", e.id);
            }
        }
    }

    #[test]
    fn planet_routes_split_inner_centers_from_outer_barycenters() {
        let es = entries();
        let expected = [
            ("mercury", "199"),
            ("venus", "299"),
            ("earth", "399"),
            ("mars", "499"),
            ("jupiter", "5"),
            ("saturn", "6"),
            ("uranus", "7"),
            ("neptune", "8"),
        ];

        for (id, expected_command) in expected {
            let entry = es
                .iter()
                .find(|entry| entry.id == id)
                .unwrap_or_else(|| panic!("missing planet '{id}'"));
            assert_eq!(
                entry.route,
                Route::HorizonsPlanet {
                    command: expected_command,
                },
                "unexpected Horizons route for '{id}'"
            );
        }
    }
}
