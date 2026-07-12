//! `xtask` — offline development tooling (Rev B §4.2 / WP3).
//! The app never touches the network; this tool does, once, at dev time.

pub mod emit;
pub mod fetch;
pub mod horizons;
pub mod manifest;
pub mod normalize;
pub mod sbdb;

use anyhow::{anyhow, bail, Context, Result};
use fetch::Fetch;
use manifest::{Entry, Route};
use sim_core::catalog::{BodyRecord, Catalog, FRAME_ECLIPJ2000, J2000_JD_TDB, SCHEMA_VERSION};

/// Rev B default start epoch: 2026-01-01 12:00 TDB (J2000 + 9497 days).
pub const DEFAULT_EPOCH_JD_TDB: f64 = 2_461_042.0;

/// Coarse sampling years for the planet secular fit (soft time range 1800–2300).
pub const SECULAR_SAMPLE_YEARS: [f64; 11] = [
    1800.0, 1850.0, 1900.0, 1950.0, 2000.0, 2050.0, 2100.0, 2150.0, 2200.0, 2250.0, 2300.0,
];

pub struct GenOptions {
    pub epoch_jd_tdb: f64,
    /// Skip bodies whose source is unavailable (fixtures mode) instead of failing.
    pub allow_partial: bool,
}

impl Default for GenOptions {
    fn default() -> Self {
        Self {
            epoch_jd_tdb: DEFAULT_EPOCH_JD_TDB,
            allow_partial: false,
        }
    }
}

/// Approximate JD for Jan 1 of `year` at noon TT — sampling grid only, so
/// calendar-exactness is irrelevant.
fn jd_of_year(year: f64) -> f64 {
    J2000_JD_TDB + (year - 2000.0) * 365.25
}

/// TLIST for a planet: coarse secular grid + epoch + epoch+1d (mean-motion fit).
pub fn planet_tlist(epoch_jd: f64) -> Vec<f64> {
    let mut t: Vec<f64> = SECULAR_SAMPLE_YEARS
        .iter()
        .map(|&y| jd_of_year(y))
        .collect();
    t.push(epoch_jd);
    t.push(epoch_jd + 1.0);
    t
}

pub struct PlanRow {
    pub id: &'static str,
    pub category: &'static str,
    pub what: String,
}

/// The fetch plan for `--dry-run`: one row per body, no network.
pub fn plan(epoch_jd: f64) -> Vec<PlanRow> {
    manifest::entries()
        .iter()
        .map(|e| {
            let what = match e.route {
                Route::SunFixed => "constants only (no fetch)".to_string(),
                Route::HorizonsPlanet { command } => format!(
                    "Horizons ELEMENTS cmd={command} center=500@10, {} epochs",
                    planet_tlist(epoch_jd).len()
                ),
                Route::HorizonsMoon { command, center } => {
                    format!("Horizons ELEMENTS cmd={command} center={center}, 1 epoch")
                }
                Route::HorizonsLookupMoon { sstr, center_hint } => {
                    format!("Horizons lookup '{sstr}' then ELEMENTS ({center_hint})")
                }
                Route::Sbdb { sstr } => format!("SBDB sstr='{sstr}'"),
            };
            PlanRow {
                id: e.id,
                category: match e.category {
                    sim_core::catalog::Category::Star => "star",
                    sim_core::catalog::Category::Planet => "planet",
                    sim_core::catalog::Category::DwarfPlanet => "dwarf",
                    sim_core::catalog::Category::Asteroid => "asteroid",
                    sim_core::catalog::Category::Moon => "moon",
                    sim_core::catalog::Category::Comet => "comet",
                },
                what,
            }
        })
        .collect()
}

fn record_shell(e: &Entry) -> BodyRecord {
    BodyRecord {
        id: e.id.to_string(),
        name: e.name.to_string(),
        designation: e.designation.map(str::to_string),
        aliases: e.aliases.iter().map(|s| s.to_string()).collect(),
        category: e.category,
        parent: e.parent.map(str::to_string),
        gm_km3_s2: e.gm_km3_s2,
        radius_km: e.radius_km,
        color_srgb: e.color,
        texture: None,
        description: e.blurb.to_string(),
        orbit: None,
        source: manifest::source_string(e),
    }
}

/// Generate the catalog through any `Fetch` source. Returns the validated
/// catalog plus the ids that were skipped (partial mode only).
pub fn generate(fetcher: &dyn Fetch, opts: &GenOptions) -> Result<(Catalog, Vec<String>)> {
    let epoch = opts.epoch_jd_tdb;
    let mut bodies: Vec<BodyRecord> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for e in manifest::entries() {
        let mut rec = record_shell(&e);
        match e.route {
            Route::SunFixed => {}
            Route::HorizonsPlanet { command } => {
                if opts.allow_partial && !fetcher.has(e.id) {
                    skipped.push(e.id.to_string());
                    continue;
                }
                let url = horizons::elements_url(command, "500@10", &planet_tlist(epoch));
                let body = fetcher
                    .get(&url, e.id)
                    .with_context(|| format!("fetch {}", e.id))?;
                let recs = horizons::parse_response(&body)
                    .with_context(|| format!("parse Horizons response for {}", e.id))?;
                rec.orbit = Some(normalize::fit_planet(&recs, epoch)?.orbit);
            }
            Route::HorizonsMoon { command, center }
            | Route::HorizonsLookupMoon {
                sstr: command,
                center_hint: center,
            } => {
                if opts.allow_partial && !fetcher.has(e.id) {
                    skipped.push(e.id.to_string());
                    continue;
                }
                if matches!(e.route, Route::HorizonsLookupMoon { .. }) && !fetcher.has(e.id) {
                    // Online lookup resolution is an open item (spec §8);
                    // fixtures satisfy the route directly.
                    bail!(
                        "'{}': Horizons lookup route not yet implemented online \
                         (resolve '{command}' near {center}); provide a fixture or \
                         see docs/wp3-gen-catalog-spec.md §Open items",
                        e.id
                    );
                }
                let url = horizons::elements_url(command, center, &[epoch]);
                let body = fetcher
                    .get(&url, e.id)
                    .with_context(|| format!("fetch {}", e.id))?;
                let recs = horizons::parse_response(&body)
                    .with_context(|| format!("parse Horizons response for {}", e.id))?;
                rec.orbit = Some(normalize::moon_orbit(&recs, epoch)?);
            }
            Route::Sbdb { sstr } => {
                if opts.allow_partial && !fetcher.has(e.id) {
                    skipped.push(e.id.to_string());
                    continue;
                }
                let url = sbdb::sbdb_url(sstr);
                let body = fetcher
                    .get(&url, e.id)
                    .with_context(|| format!("fetch {}", e.id))?;
                let parsed = sbdb::parse_response(&body)
                    .with_context(|| format!("parse SBDB for {}", e.id))?;
                rec.orbit =
                    Some(sbdb::to_orbit(&parsed).with_context(|| format!("normalize {}", e.id))?);
            }
        }
        bodies.push(rec);
    }

    // Partial mode: drop bodies whose parent chain got skipped (orphans).
    if opts.allow_partial {
        loop {
            let ids: std::collections::HashSet<String> =
                bodies.iter().map(|b| b.id.clone()).collect();
            let before = bodies.len();
            bodies.retain(|b| match &b.parent {
                Some(p) => {
                    let keep = ids.contains(p);
                    if !keep {
                        skipped.push(b.id.clone());
                    }
                    keep
                }
                None => true,
            });
            if bodies.len() == before {
                break;
            }
        }
    }

    let catalog = Catalog {
        schema_version: SCHEMA_VERSION,
        generated_utc: emit::now_utc_iso8601(),
        frame: FRAME_ECLIPJ2000.to_string(),
        bodies,
    };

    if let Err(errs) = catalog.validate() {
        let joined = errs
            .iter()
            .map(|e| format!("  - {e}"))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(anyhow!("generated catalog failed validation:\n{joined}"));
    }
    Ok((catalog, skipped))
}
