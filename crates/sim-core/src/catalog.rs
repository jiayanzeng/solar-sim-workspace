//! `catalog.ron` schema and load-time validation (Rev B §4, WP3).
//!
//! Design contract (see `docs/wp3-gen-catalog-spec.md` for the full spec):
//! - The file is *generated* by `xtask gen-catalog`, committed to the repo,
//!   and loaded by the app at startup. The app never touches the network.
//! - Frame: heliocentric (planets/dwarfs/asteroids/comets) or parent-centric
//!   (moons) osculating elements in the **ecliptic J2000** frame ("ECLIPJ2000").
//! - Units in the file are human-auditable: km, degrees, Julian Date (TDB).
//!   Runtime converts to radians/seconds once at load.
//! - Hyperbolic orbits (3I/ATLAS) are carried in the same `Elements` struct
//!   with `a_km < 0` and `e > 1`. Parabolic (e == 1) is rejected.
//! - Validation is CI-grade: it collects *all* errors, not just the first,
//!   and rejects non-finite, parentless, duplicate, or frame-inconsistent data.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Astronomical unit, km (IAU 2012 definition).
pub const AU_KM: f64 = 149_597_870.7;
/// J2000.0 epoch as Julian Date (TDB).
pub const J2000_JD_TDB: f64 = 2_451_545.0;
pub const SECONDS_PER_DAY: f64 = 86_400.0;
pub const DAYS_PER_JULIAN_CENTURY: f64 = 36_525.0;

/// Bump on any breaking schema change; validation hard-fails on mismatch.
pub const SCHEMA_VERSION: u32 = 1;
/// The only frame accepted by schema v1.
pub const FRAME_ECLIPJ2000: &str = "ECLIPJ2000";

/// Sanity window for element epochs (~JD 2200000 ≈ 1310 CE, 2600000 ≈ 2406 CE).
/// Comet epochs are re-based to perihelion passage, so the window is generous.
const EPOCH_JD_MIN: f64 = 2_200_000.0;
const EPOCH_JD_MAX: f64 = 2_600_000.0;

// ---------------------------------------------------------------------------
// Schema types
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Star,
    Planet,
    DwarfPlanet,
    Asteroid,
    Moon,
    Comet,
}

impl Category {
    /// Categories whose parent must be the (single) star.
    pub fn orbits_star(self) -> bool {
        matches!(
            self,
            Category::Planet | Category::DwarfPlanet | Category::Asteroid | Category::Comet
        )
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Category::Star => "Star",
            Category::Planet => "Planet",
            Category::DwarfPlanet => "Dwarf Planet",
            Category::Asteroid => "Asteroid",
            Category::Moon => "Moon",
            Category::Comet => "Comet",
        };
        f.write_str(s)
    }
}

/// Osculating Keplerian elements at `Orbit::epoch_jd_tdb`, in the parent-centric
/// ecliptic-J2000 frame. Angles in degrees, semi-major axis in km.
///
/// Conventions:
/// - `a_km > 0`, `0 <= e < 1`  → elliptic
/// - `a_km < 0`, `e > 1`       → hyperbolic (open arc; 3I/ATLAS)
/// - `m0_deg` is the mean anomaly at epoch. For hyperbolic orbits it is the
///   *hyperbolic* mean anomaly M = n·(t − tp); the generator re-bases comet
///   epochs to perihelion so `m0_deg == 0` there by construction.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Elements {
    pub a_km: f64,
    pub e: f64,
    pub i_deg: f64,
    pub raan_deg: f64,
    pub argp_deg: f64,
    pub m0_deg: f64,
}

impl Elements {
    pub fn is_hyperbolic(&self) -> bool {
        self.e > 1.0
    }
    /// Perihelion distance, km (valid for both branches).
    pub fn periapsis_km(&self) -> f64 {
        self.a_km * (1.0 - self.e)
    }
}

/// Linear secular rates per Julian century, applied about the element epoch.
/// Emitted for planets only (fit from Horizons sampling across 1800–2300);
/// moons, small bodies, and comets carry `None`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct SecularRates {
    pub a_km_per_cy: f64,
    pub e_per_cy: f64,
    pub i_deg_per_cy: f64,
    pub raan_deg_per_cy: f64,
    pub argp_deg_per_cy: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Orbit {
    /// Element epoch, Julian Date in TDB.
    pub epoch_jd_tdb: f64,
    pub elements: Elements,
    #[serde(default)]
    pub secular: Option<SecularRates>,
    /// Effective mean motion fitted from ephemeris sampling (deg/day).
    /// When present it overrides `sqrt(mu/|a|^3)` — it captures the
    /// perturbation-averaged motion the way Standish's mean-longitude rates do.
    #[serde(default)]
    pub mean_motion_deg_per_day: Option<f64>,
}

impl Orbit {
    /// Mean motion in rad/s: fitted override if present, else two-body from
    /// the parent's gravitational parameter.
    pub fn mean_motion_rad_per_s(&self, mu_parent_km3_s2: f64) -> f64 {
        match self.mean_motion_deg_per_day {
            Some(n) => n.to_radians() / SECONDS_PER_DAY,
            None => (mu_parent_km3_s2 / self.elements.a_km.abs().powi(3)).sqrt(),
        }
    }

    /// Orbital period in seconds (elliptic only).
    pub fn period_s(&self, mu_parent_km3_s2: f64) -> Option<f64> {
        if self.elements.is_hyperbolic() {
            None
        } else {
            Some(std::f64::consts::TAU / self.mean_motion_rad_per_s(mu_parent_km3_s2))
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BodyRecord {
    /// Stable machine id: `[a-z0-9_]+`. Referenced by commands, settings,
    /// replay streams, and tests — never display it.
    pub id: String,
    /// Display name ("3I/ATLAS", "1P/Halley", "Io").
    pub name: String,
    /// Formal designation, if distinct from the name ("C/2025 N1").
    #[serde(default)]
    pub designation: Option<String>,
    /// Extra search aliases. Name + designation + aliases must be unique
    /// across the whole catalog (case-insensitive) — search depends on it.
    #[serde(default)]
    pub aliases: Vec<String>,
    pub category: Category,
    /// Parent body id. `None` only for the star.
    #[serde(default)]
    pub parent: Option<String>,
    /// Curated WP10 visibility tier. Meaningful only for moons; false for
    /// every other category. Missing values deserialize as false so schema-v1
    /// audit fixtures captured before WP10 remain readable.
    #[serde(default)]
    pub is_major_moon: bool,
    /// Gravitational parameter, km³/s². Required for any body that is a parent.
    #[serde(default)]
    pub gm_km3_s2: Option<f64>,
    /// Mean physical radius, km (curated; review pass per Rev B §4.2).
    pub radius_km: f64,
    /// Fallback body/material color; every body renders without a texture.
    pub color_srgb: (u8, u8, u8),
    /// Human-reviewed orbit-path color. `(0, 0, 0)` is the reserved
    /// deserialization sentinel for pre-Rev-E audit fixtures and is rejected
    /// by validation; every generated production body must provide a value.
    #[serde(default)]
    pub orbit_color_srgb: (u8, u8, u8),
    /// KTX2 texture asset path, if any (WP15 polish pass).
    #[serde(default)]
    pub texture: Option<String>,
    /// Curated description shown in the Info tab. Empty remains a non-fatal
    /// lint so pre-content-pass audit fixtures stay readable.
    #[serde(default)]
    pub description: String,
    /// Reviewed English-Wikipedia article used by the Info-panel reference
    /// action. Optional for backward compatibility with pre-Rev-E fixtures;
    /// generated production records provide exactly one validated URL.
    #[serde(default)]
    pub wikipedia_url: Option<String>,
    /// Orbit around `parent`. `None` only for the star.
    #[serde(default)]
    pub orbit: Option<Orbit>,
    /// Data provenance, human-readable. Required non-empty (licensing audit).
    pub source: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Catalog {
    pub schema_version: u32,
    /// ISO-8601 UTC timestamp written by the generator.
    pub generated_utc: String,
    /// Must be `FRAME_ECLIPJ2000` in schema v1.
    pub frame: String,
    pub bodies: Vec<BodyRecord>,
}

// ---------------------------------------------------------------------------
// Load / save
// ---------------------------------------------------------------------------

impl Catalog {
    pub fn from_ron_str(s: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(s)
    }

    pub fn to_ron_string(&self) -> Result<String, ron::Error> {
        let cfg = ron::ser::PrettyConfig::new()
            .struct_names(true)
            .indentor("  ".to_string());
        ron::ser::to_string_pretty(self, cfg)
    }

    /// Index of body ids to positions in `bodies`.
    pub fn id_index(&self) -> HashMap<&str, usize> {
        self.bodies
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id.as_str(), i))
            .collect()
    }

    /// Case-insensitive exact lookup across name, designation, and aliases —
    /// the seed of the WP12 fuzzy search ("3I/ATLAS" and "C/2025 N1" must both hit).
    pub fn find(&self, query: &str) -> Option<&BodyRecord> {
        let q = query.trim().to_lowercase();
        self.bodies.iter().find(|b| {
            b.name.to_lowercase() == q
                || b.id == q
                || b.designation.as_deref().map(str::to_lowercase) == Some(q.clone())
                || b.aliases.iter().any(|a| a.to_lowercase() == q)
        })
    }

    pub fn counts_by_category(&self) -> HashMap<Category, usize> {
        let mut m = HashMap::new();
        for b in &self.bodies {
            *m.entry(b.category).or_insert(0) += 1;
        }
        m
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum CatalogError {
    SchemaVersion {
        found: u32,
        expected: u32,
    },
    BadFrame {
        found: String,
    },
    EmptyId,
    BadIdChars {
        id: String,
    },
    DuplicateId {
        id: String,
    },
    DuplicateSearchKey {
        key: String,
        first: String,
        second: String,
    },
    MissingOrbitColor {
        id: String,
    },
    DuplicateOrbitColor {
        color: (u8, u8, u8),
        first: String,
        second: String,
    },
    StarCount {
        found: usize,
    },
    StarHasParentOrOrbit {
        id: String,
    },
    MissingParent {
        id: String,
    },
    UnknownParent {
        id: String,
        parent: String,
    },
    ParentCycle {
        id: String,
    },
    MissingOrbit {
        id: String,
    },
    HeliocentricParentNotStar {
        id: String,
        parent: String,
    },
    MoonParentIsStar {
        id: String,
    },
    NonMoonMarkedMajor {
        id: String,
    },
    ParentMissingGm {
        parent: String,
        child: String,
    },
    NonFinite {
        id: String,
        field: &'static str,
    },
    NonPositive {
        id: String,
        field: &'static str,
    },
    EccentricityAxisMismatch {
        id: String,
        a_km: f64,
        e: f64,
    },
    ParabolicUnsupported {
        id: String,
    },
    EpochOutOfRange {
        id: String,
        jd: f64,
    },
    MeanMotionInvalid {
        id: String,
    },
    EmptySource {
        id: String,
    },
    InvalidWikipediaUrl {
        id: String,
        url: String,
    },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use CatalogError::*;
        match self {
            SchemaVersion { found, expected } => {
                write!(f, "schema_version {found} != expected {expected}")
            }
            BadFrame { found } => write!(f, "frame '{found}' != '{FRAME_ECLIPJ2000}'"),
            EmptyId => write!(f, "body with empty id"),
            BadIdChars { id } => write!(f, "id '{id}' must match [a-z0-9_]+"),
            DuplicateId { id } => write!(f, "duplicate id '{id}'"),
            DuplicateSearchKey { key, first, second } => write!(
                f,
                "search key '{key}' collides between '{first}' and '{second}'"
            ),
            MissingOrbitColor { id } => write!(
                f,
                "'{id}': orbit_color_srgb is missing or reserved black (0, 0, 0)"
            ),
            DuplicateOrbitColor {
                color,
                first,
                second,
            } => write!(
                f,
                "orbit_color_srgb {color:?} is duplicated by '{first}' and '{second}'"
            ),
            StarCount { found } => write!(f, "catalog must contain exactly 1 Star, found {found}"),
            StarHasParentOrOrbit { id } => {
                write!(f, "star '{id}' must have no parent and no orbit")
            }
            MissingParent { id } => write!(f, "'{id}' is not a star but has no parent"),
            UnknownParent { id, parent } => {
                write!(f, "'{id}' references unknown parent '{parent}'")
            }
            ParentCycle { id } => write!(f, "parent chain of '{id}' does not terminate at a star"),
            MissingOrbit { id } => write!(f, "'{id}' is not a star but has no orbit"),
            HeliocentricParentNotStar { id, parent } => write!(
                f,
                "'{id}' (heliocentric category) must orbit the star, not '{parent}'"
            ),
            MoonParentIsStar { id } => write!(f, "moon '{id}' must not orbit the star directly"),
            NonMoonMarkedMajor { id } => {
                write!(f, "'{id}' is marked as a major moon but is not a moon")
            }
            ParentMissingGm { parent, child } => write!(
                f,
                "'{parent}' has child '{child}' but no gm_km3_s2 (mean motion needs it)"
            ),
            NonFinite { id, field } => write!(f, "'{id}': field '{field}' is not finite"),
            NonPositive { id, field } => write!(f, "'{id}': field '{field}' must be > 0"),
            EccentricityAxisMismatch { id, a_km, e } => write!(
                f,
                "'{id}': sign(a)={a_km:+.3e} inconsistent with e={e} (elliptic needs a>0, hyperbolic a<0)"
            ),
            ParabolicUnsupported { id } => {
                write!(f, "'{id}': e == 1 (parabolic) is not supported by schema v1")
            }
            EpochOutOfRange { id, jd } => write!(
                f,
                "'{id}': epoch JD {jd} outside sanity window [{EPOCH_JD_MIN}, {EPOCH_JD_MAX}]"
            ),
            MeanMotionInvalid { id } => {
                write!(f, "'{id}': mean_motion_deg_per_day must be finite and > 0")
            }
            EmptySource { id } => write!(f, "'{id}': source must be non-empty (licensing audit)"),
            InvalidWikipediaUrl { id, url } => write!(
                f,
                "'{id}': wikipedia_url '{url}' must be an HTTPS en.wikipedia.org/wiki/ article URL with a non-empty slug"
            ),
        }
    }
}

/// Return whether `url` is a direct English-Wikipedia article URL approved
/// for catalog storage and later platform dispatch.
///
/// This deliberately avoids a general URL parser: the catalog contract is a
/// single exact origin and path prefix, and `sim-core`'s dependency set is
/// frozen. Query strings, fragments, whitespace, and control characters are
/// excluded so UI/platform code never receives an ambiguous target.
pub fn is_valid_wikipedia_url(url: &str) -> bool {
    const PREFIX: &str = "https://en.wikipedia.org/wiki/";
    let Some(slug) = url.strip_prefix(PREFIX) else {
        return false;
    };
    !slug.is_empty()
        && slug != "/"
        && !slug.starts_with('/')
        && !slug.chars().any(char::is_whitespace)
        && !slug.chars().any(char::is_control)
        && !slug.contains('?')
        && !slug.contains('#')
}

impl Catalog {
    /// Full structural validation. Collects every error (CI wants the list,
    /// not the first failure). `Ok(())` means the catalog is safe to load.
    pub fn validate(&self) -> Result<(), Vec<CatalogError>> {
        let mut errs = Vec::new();

        if self.schema_version != SCHEMA_VERSION {
            errs.push(CatalogError::SchemaVersion {
                found: self.schema_version,
                expected: SCHEMA_VERSION,
            });
        }
        if self.frame != FRAME_ECLIPJ2000 {
            errs.push(CatalogError::BadFrame {
                found: self.frame.clone(),
            });
        }

        // --- ids ---
        let mut index: HashMap<&str, usize> = HashMap::new();
        for (i, b) in self.bodies.iter().enumerate() {
            if b.id.is_empty() {
                errs.push(CatalogError::EmptyId);
                continue;
            }
            if !b
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                errs.push(CatalogError::BadIdChars { id: b.id.clone() });
            }
            if index.insert(b.id.as_str(), i).is_some() {
                errs.push(CatalogError::DuplicateId { id: b.id.clone() });
            }
        }

        // --- search-key uniqueness (name + designation + aliases, casefolded) ---
        let mut keys: HashMap<String, String> = HashMap::new();
        for b in &self.bodies {
            let mut body_keys: Vec<String> = vec![b.name.to_lowercase()];
            if let Some(d) = &b.designation {
                body_keys.push(d.to_lowercase());
            }
            body_keys.extend(b.aliases.iter().map(|a| a.to_lowercase()));
            for k in body_keys {
                if k.is_empty() {
                    continue;
                }
                if let Some(first) = keys.get(&k) {
                    if first != &b.id {
                        errs.push(CatalogError::DuplicateSearchKey {
                            key: k.clone(),
                            first: first.clone(),
                            second: b.id.clone(),
                        });
                    }
                } else {
                    keys.insert(k, b.id.clone());
                }
            }
        }

        // --- star topology ---
        let stars: Vec<&BodyRecord> = self
            .bodies
            .iter()
            .filter(|b| b.category == Category::Star)
            .collect();
        if stars.len() != 1 {
            errs.push(CatalogError::StarCount { found: stars.len() });
        }
        let star_id = stars.first().map(|s| s.id.clone());

        // --- per-body checks ---
        let has_children: std::collections::HashSet<&str> = self
            .bodies
            .iter()
            .filter_map(|b| b.parent.as_deref())
            .collect();
        let mut orbit_colors: HashMap<(u8, u8, u8), String> = HashMap::new();

        for b in &self.bodies {
            let id = || b.id.clone();

            if b.source.trim().is_empty() {
                errs.push(CatalogError::EmptySource { id: id() });
            }
            if let Some(url) = &b.wikipedia_url {
                if !is_valid_wikipedia_url(url) {
                    errs.push(CatalogError::InvalidWikipediaUrl {
                        id: id(),
                        url: url.clone(),
                    });
                }
            }
            if b.orbit_color_srgb == (0, 0, 0) {
                errs.push(CatalogError::MissingOrbitColor { id: id() });
            } else if b.category != Category::Star {
                if let Some(first) = orbit_colors.insert(b.orbit_color_srgb, b.id.clone()) {
                    errs.push(CatalogError::DuplicateOrbitColor {
                        color: b.orbit_color_srgb,
                        first,
                        second: b.id.clone(),
                    });
                }
            }
            if !b.radius_km.is_finite() {
                errs.push(CatalogError::NonFinite {
                    id: id(),
                    field: "radius_km",
                });
            } else if b.radius_km <= 0.0 {
                errs.push(CatalogError::NonPositive {
                    id: id(),
                    field: "radius_km",
                });
            }
            if let Some(gm) = b.gm_km3_s2 {
                if !gm.is_finite() {
                    errs.push(CatalogError::NonFinite {
                        id: id(),
                        field: "gm_km3_s2",
                    });
                } else if gm <= 0.0 {
                    errs.push(CatalogError::NonPositive {
                        id: id(),
                        field: "gm_km3_s2",
                    });
                }
            }
            if has_children.contains(b.id.as_str()) && b.gm_km3_s2.is_none() {
                // report once per parent using first child for context
                if let Some(child) = self
                    .bodies
                    .iter()
                    .find(|c| c.parent.as_deref() == Some(b.id.as_str()))
                {
                    errs.push(CatalogError::ParentMissingGm {
                        parent: b.id.clone(),
                        child: child.id.clone(),
                    });
                }
            }
            if b.is_major_moon && b.category != Category::Moon {
                errs.push(CatalogError::NonMoonMarkedMajor { id: id() });
            }

            if b.category == Category::Star {
                if b.parent.is_some() || b.orbit.is_some() {
                    errs.push(CatalogError::StarHasParentOrOrbit { id: id() });
                }
                continue;
            }

            // non-star topology
            match &b.parent {
                None => errs.push(CatalogError::MissingParent { id: id() }),
                Some(p) => {
                    if !index.contains_key(p.as_str()) {
                        errs.push(CatalogError::UnknownParent {
                            id: id(),
                            parent: p.clone(),
                        });
                    } else {
                        if b.category.orbits_star() {
                            if Some(p.clone()) != star_id {
                                errs.push(CatalogError::HeliocentricParentNotStar {
                                    id: id(),
                                    parent: p.clone(),
                                });
                            }
                        } else if b.category == Category::Moon && Some(p.clone()) == star_id {
                            errs.push(CatalogError::MoonParentIsStar { id: id() });
                        }
                        // cycle check: walk to root, bounded by body count
                        let mut cur = p.as_str();
                        let mut steps = 0usize;
                        let terminated = loop {
                            match index.get(cur).map(|&ix| &self.bodies[ix]) {
                                Some(node) => {
                                    if node.category == Category::Star {
                                        break true;
                                    }
                                    match node.parent.as_deref() {
                                        Some(next) => {
                                            cur = next;
                                            steps += 1;
                                            if steps > self.bodies.len() {
                                                break false;
                                            }
                                        }
                                        None => break false,
                                    }
                                }
                                None => break false, // unknown parent already reported
                            }
                        };
                        if !terminated && index.contains_key(p.as_str()) {
                            errs.push(CatalogError::ParentCycle { id: id() });
                        }
                    }
                }
            }

            // orbit checks
            match &b.orbit {
                None => errs.push(CatalogError::MissingOrbit { id: id() }),
                Some(o) => {
                    let el = &o.elements;
                    let fields: [(&'static str, f64); 8] = [
                        ("epoch_jd_tdb", o.epoch_jd_tdb),
                        ("a_km", el.a_km),
                        ("e", el.e),
                        ("i_deg", el.i_deg),
                        ("raan_deg", el.raan_deg),
                        ("argp_deg", el.argp_deg),
                        ("m0_deg", el.m0_deg),
                        (
                            "secular/mean_motion",
                            o.mean_motion_deg_per_day.unwrap_or(1.0)
                                + o.secular.map(|s| s.a_km_per_cy).unwrap_or(0.0),
                        ),
                    ];
                    let mut finite = true;
                    for (name, v) in fields {
                        if !v.is_finite() {
                            errs.push(CatalogError::NonFinite {
                                id: id(),
                                field: name,
                            });
                            finite = false;
                        }
                    }
                    if finite {
                        if el.e < 0.0 {
                            errs.push(CatalogError::NonPositive {
                                id: id(),
                                field: "e",
                            });
                        } else if (el.e - 1.0).abs() < 1e-9 {
                            errs.push(CatalogError::ParabolicUnsupported { id: id() });
                        } else if (el.e < 1.0 && el.a_km <= 0.0) || (el.e > 1.0 && el.a_km >= 0.0) {
                            errs.push(CatalogError::EccentricityAxisMismatch {
                                id: id(),
                                a_km: el.a_km,
                                e: el.e,
                            });
                        }
                        if o.epoch_jd_tdb < EPOCH_JD_MIN || o.epoch_jd_tdb > EPOCH_JD_MAX {
                            errs.push(CatalogError::EpochOutOfRange {
                                id: id(),
                                jd: o.epoch_jd_tdb,
                            });
                        }
                        if let Some(n) = o.mean_motion_deg_per_day {
                            if !n.is_finite() || n <= 0.0 {
                                errs.push(CatalogError::MeanMotionInvalid { id: id() });
                            }
                        }
                    }
                }
            }
        }

        if errs.is_empty() {
            Ok(())
        } else {
            Err(errs)
        }
    }

    /// Non-fatal quality lints (content-pass debt, not load blockers).
    pub fn lint(&self) -> Vec<String> {
        let mut out = Vec::new();
        for b in &self.bodies {
            if b.description.trim().is_empty() {
                out.push(format!(
                    "'{}': description is empty (WP10 content pass)",
                    b.id
                ));
            }
            if b.texture.is_none() && matches!(b.category, Category::Star | Category::Planet) {
                out.push(format!(
                    "'{}': no texture assigned (WP15 polish pass)",
                    b.id
                ));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A miniature but structurally complete catalog exercising both the
    /// elliptic and hyperbolic branches, a moon, and alias search.
    const SAMPLE_RON: &str = r#"
// generated by xtask gen-catalog (sample) — comments are legal RON
Catalog(
    schema_version: 1,
    generated_utc: "2026-07-12T00:00:00Z",
    frame: "ECLIPJ2000",
    bodies: [
        BodyRecord(
            id: "sun",
            name: "Sun",
            aliases: ["Sol"],
            category: Star,
            gm_km3_s2: Some(1.32712440018e11),
            radius_km: 695700.0,
            color_srgb: (255, 214, 140),
            orbit_color_srgb: (255, 214, 140),
            description: "The star at the center of the solar system.",
            source: "IAU nominal constants",
        ),
        BodyRecord(
            id: "earth",
            name: "Earth",
            category: Planet,
            parent: Some("sun"),
            gm_km3_s2: Some(3.986004418e5),
            radius_km: 6371.0,
            color_srgb: (86, 141, 235),
            orbit_color_srgb: (77, 141, 255),
            description: "Our home planet.",
            orbit: Some(Orbit(
                epoch_jd_tdb: 2461042.0,
                elements: Elements(
                    a_km: 1.4959787e8,
                    e: 0.0167,
                    i_deg: 0.003,
                    raan_deg: 175.0,
                    argp_deg: 288.0,
                    m0_deg: 357.5,
                ),
                secular: Some(SecularRates(
                    a_km_per_cy: -20.0,
                    e_per_cy: -0.00004,
                    i_deg_per_cy: -0.01,
                    raan_deg_per_cy: -0.24,
                    argp_deg_per_cy: 0.32,
                )),
                mean_motion_deg_per_day: Some(0.9856),
            )),
            source: "JPL Horizons ELEMENTS, heliocentric ecliptic J2000",
        ),
        BodyRecord(
            id: "moon",
            name: "Moon",
            aliases: ["Luna"],
            category: Moon,
            parent: Some("earth"),
            radius_km: 1737.4,
            color_srgb: (198, 189, 175),
            orbit_color_srgb: (199, 194, 184),
            orbit: Some(Orbit(
                epoch_jd_tdb: 2461042.0,
                elements: Elements(
                    a_km: 384400.0,
                    e: 0.0549,
                    i_deg: 5.145,
                    raan_deg: 125.0,
                    argp_deg: 318.0,
                    m0_deg: 135.0,
                ),
            )),
            source: "JPL Horizons ELEMENTS, geocentric ecliptic J2000",
        ),
        BodyRecord(
            id: "3i_atlas",
            name: "3I/ATLAS",
            designation: Some("C/2025 N1"),
            aliases: ["3I"],
            category: Comet,
            parent: Some("sun"),
            radius_km: 2.5,
            color_srgb: (166, 216, 232),
            orbit_color_srgb: (116, 166, 201),
            orbit: Some(Orbit(
                epoch_jd_tdb: 2460978.0,
                elements: Elements(
                    a_km: -3.99e7,
                    e: 6.1,
                    i_deg: 175.1,
                    raan_deg: 322.0,
                    argp_deg: 128.0,
                    m0_deg: 0.0,
                ),
            )),
            source: "JPL SBDB (interstellar; epoch re-based to perihelion)",
        ),
    ],
)
"#;

    fn sample() -> Catalog {
        Catalog::from_ron_str(SAMPLE_RON).expect("sample RON must parse")
    }

    #[test]
    fn sample_parses_and_validates() {
        let c = sample();
        assert_eq!(c.bodies.len(), 4);
        c.validate().expect("sample must be valid");
    }

    #[test]
    fn ron_round_trip() {
        let c = sample();
        let s = c.to_ron_string().unwrap();
        let c2 = Catalog::from_ron_str(&s).unwrap();
        c2.validate().unwrap();
        assert_eq!(c2.bodies.len(), c.bodies.len());
        assert_eq!(c2.bodies[3].orbit, c.bodies[3].orbit);
        assert!(
            c2.bodies.iter().all(|body| body.wikipedia_url.is_none()),
            "legacy RON must default the additive field to None"
        );
    }

    #[test]
    fn alias_search_hits_designation_and_alias() {
        let c = sample();
        assert_eq!(c.find("c/2025 n1").unwrap().id, "3i_atlas");
        assert_eq!(c.find("3I/ATLAS").unwrap().id, "3i_atlas");
        assert_eq!(c.find("luna").unwrap().id, "moon");
        assert!(c.find("voyager").is_none());
    }

    #[test]
    fn hyperbolic_helpers() {
        let c = sample();
        let atlas = c.find("3I").unwrap();
        let o = atlas.orbit.as_ref().unwrap();
        assert!(o.elements.is_hyperbolic());
        assert!(o.elements.periapsis_km() > 0.0);
        assert!(o.period_s(1.327e11).is_none());
        assert!(o.mean_motion_rad_per_s(1.327e11) > 0.0);
    }

    #[test]
    fn rejects_duplicate_id() {
        let mut c = sample();
        let dup = c.bodies[1].clone();
        c.bodies.push(dup);
        let errs = c.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, CatalogError::DuplicateId { .. })));
    }

    #[test]
    fn rejects_missing_and_duplicate_orbit_colors_together() {
        let mut c = sample();
        c.bodies[1].orbit_color_srgb = (0, 0, 0);
        c.bodies[3].orbit_color_srgb = c.bodies[2].orbit_color_srgb;
        let errs = c.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, CatalogError::MissingOrbitColor { id } if id == "earth")));
        assert!(errs.iter().any(|e| matches!(
            e,
            CatalogError::DuplicateOrbitColor { first, second, .. }
                if first == "moon" && second == "3i_atlas"
        )));
    }

    #[test]
    fn wikipedia_urls_accept_only_direct_https_english_articles() {
        let valid = [
            "https://en.wikipedia.org/wiki/Earth",
            "https://en.wikipedia.org/wiki/67P/Churyumov%E2%80%93Gerasimenko",
            "https://en.wikipedia.org/wiki/Hi%CA%BBiaka_(moon)",
        ];
        for url in valid {
            assert!(is_valid_wikipedia_url(url), "{url}");
        }

        let invalid = [
            "",
            "http://en.wikipedia.org/wiki/Earth",
            "https://wikipedia.org/wiki/Earth",
            "https://de.wikipedia.org/wiki/Erde",
            "https://en.wikipedia.org/wiki/",
            "https://en.wikipedia.org/wiki//",
            "https://en.wikipedia.org/wiki/Earth?oldid=1",
            "https://en.wikipedia.org/wiki/Earth#History",
            "https://en.wikipedia.org/wiki/Earth orbit",
        ];
        for url in invalid {
            assert!(!is_valid_wikipedia_url(url), "{url}");
        }
    }

    #[test]
    fn validation_collects_invalid_wikipedia_urls() {
        let mut c = sample();
        c.bodies[0].wikipedia_url = Some("http://en.wikipedia.org/wiki/Sun".into());
        c.bodies[1].wikipedia_url = Some("https://example.com/Earth".into());
        let errs = c.validate().unwrap_err();
        let ids: Vec<&str> = errs
            .iter()
            .filter_map(|error| match error {
                CatalogError::InvalidWikipediaUrl { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, ["sun", "earth"]);
    }

    #[test]
    fn rejects_parentless_planet() {
        let mut c = sample();
        c.bodies[1].parent = None;
        let errs = c.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, CatalogError::MissingParent { .. })));
    }

    #[test]
    fn rejects_unknown_parent() {
        let mut c = sample();
        c.bodies[2].parent = Some("jupiter".into());
        let errs = c.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, CatalogError::UnknownParent { .. })));
    }

    #[test]
    fn rejects_nonfinite_elements() {
        let mut c = sample();
        c.bodies[1].orbit.as_mut().unwrap().elements.a_km = f64::NAN;
        let errs = c.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, CatalogError::NonFinite { .. })));
    }

    #[test]
    fn rejects_axis_eccentricity_mismatch() {
        let mut c = sample();
        // hyperbolic e with positive a
        c.bodies[3].orbit.as_mut().unwrap().elements.a_km = 3.99e7;
        let errs = c.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, CatalogError::EccentricityAxisMismatch { .. })));
    }

    #[test]
    fn rejects_parabolic() {
        let mut c = sample();
        c.bodies[3].orbit.as_mut().unwrap().elements.e = 1.0;
        let errs = c.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, CatalogError::ParabolicUnsupported { .. })));
    }

    #[test]
    fn rejects_alias_collision_case_insensitive() {
        let mut c = sample();
        c.bodies[2].aliases.push("EARTH".into());
        let errs = c.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, CatalogError::DuplicateSearchKey { .. })));
    }

    #[test]
    fn rejects_parent_without_gm() {
        let mut c = sample();
        c.bodies[1].gm_km3_s2 = None; // earth has moon child
        let errs = c.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, CatalogError::ParentMissingGm { .. })));
    }

    #[test]
    fn rejects_moon_orbiting_star() {
        let mut c = sample();
        c.bodies[2].parent = Some("sun".into());
        let errs = c.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, CatalogError::MoonParentIsStar { .. })));
    }

    #[test]
    fn major_moon_flag_defaults_false_and_rejects_non_moons() {
        let mut c = sample();
        assert!(!c.bodies[2].is_major_moon, "legacy RON defaults to false");
        c.bodies[1].is_major_moon = true;
        let errs = c.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, CatalogError::NonMoonMarkedMajor { .. })));
    }

    #[test]
    fn rejects_wrong_schema_version_and_frame() {
        let mut c = sample();
        c.schema_version = 99;
        c.frame = "EQJ2000".into();
        let errs = c.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, CatalogError::SchemaVersion { .. })));
        assert!(errs
            .iter()
            .any(|e| matches!(e, CatalogError::BadFrame { .. })));
    }

    #[test]
    fn corrupt_ron_is_rejected_by_loader() {
        // WP3 acceptance: "loader rejects corrupt fixtures"
        assert!(Catalog::from_ron_str("Catalog(schema_version: 1,").is_err());
        assert!(Catalog::from_ron_str("42").is_err());
    }

    #[test]
    fn lints_flag_missing_descriptions() {
        let c = sample();
        let lints = c.lint();
        assert!(lints.iter().any(|l| l.contains("moon")));
    }
}
