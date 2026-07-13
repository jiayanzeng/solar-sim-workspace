# WP3 — `catalog.ron` Schema & `xtask gen-catalog` Specification

**Status:** Implemented (schema + validation + generator, offline smoke path,
raw-response capture/diagnostics, TNO-satellite lookup resolution, and the
66-body online capture, and radius/TNO curated-value review). The separate
Sun/planet GM review and position spot-check data remain before WP3 sign-off,
itemized in §8–§9.
**Parent design:** Rev B §4 (Body Catalog), §7 (workspace/firewall), §11 (WP3 acceptance).
**Companion code:** `crates/sim-core/src/catalog.rs` (schema of record), `xtask/src/*` (generator).

The Rust types in `sim-core::catalog` are the normative schema; this document explains the semantics, the sourcing rules, and the decisions a reviewer or coding agent needs but should never have to reverse-engineer from code.

---

## 1. Purpose and boundaries

`xtask gen-catalog` runs **once, at development time**, queries public-domain JPL services, and emits `assets/catalog.ron`, which is committed. The shipped app loads the committed file and never touches the network (Rev B §4.2). The generator is dev tooling: it lives in `xtask/`, is excluded from every shipped artifact, and its network access is behind an opt-in cargo feature (`--features online`) so default builds and CI remain fully offline.

The catalog carries exactly what WP4–WP13 consume and nothing more: identity and search aliases, taxonomy (category + parent), two-body osculating elements with optional planet secular rates, physical radius and GM, display color, description text, texture reference, and per-body provenance.

## 2. Curated vs. generated — the anti-typo split

Rev B names hand-typos across 66 bodies as the main data risk. The mitigation is a hard split of authorship:

| Curated in `xtask/src/manifest.rs` (human-reviewed) | Generated from JPL (never hand-typed) |
|---|---|
| id, name, designation, aliases | every orbital element |
| category, parent | element epoch |
| physical radius, GM (parents only) | secular rates (planets) |
| display color, description blurb | fitted mean motion (planets) |
| source route + provenance note | |

The manifest is small, diffable, and covered by its own unit tests (66 bodies, Rev B category counts 1/8/9/8/32/8, unique ids, parents precede children, every parent has a GM). All 66 radii and the three TNO parent-system GMs were human-reviewed on 2026-07-13; the radius audit and decisions are recorded in `docs/wp3-radius-audit-2026-07-13.md`. Their WP3 review markers are cleared.

## 3. The `catalog.ron` schema (v1)

File-level: `schema_version` (must be `1`), `generated_utc` (ISO-8601, written by the generator), `frame` (must be `"ECLIPJ2000"`), `bodies` (ordered list, parents before children by convention).

Frame and units: all elements are osculating Keplerian elements in the **ecliptic J2000** frame — heliocentric for planets, dwarf planets, asteroids, and comets; **parent-centric** for moons. File units are human-auditable: kilometers, degrees, Julian Date in **TDB**. The runtime converts to radians/seconds once at load; nothing downstream re-parses strings.

Per body (`BodyRecord`): `id` (stable machine key, `[a-z0-9_]+`; commands, settings, and replay streams reference ids, never names), `name`, optional `designation`, `aliases`, `category` (`Star | Planet | DwarfPlanet | Asteroid | Moon | Comet`), optional `parent` id, optional `gm_km3_s2`, `radius_km`, `color_srgb` (u8 triple), optional `texture`, `description`, optional `orbit`, and required non-empty `source`.

Orbit (`Orbit`): `epoch_jd_tdb`, `elements` (`a_km`, `e`, `i_deg`, `raan_deg`, `argp_deg`, `m0_deg`), optional `secular` (per-Julian-century linear rates on a/e/i/Ω/ω; planets only), optional `mean_motion_deg_per_day` (fitted override; see §4).

Conic conventions: `a_km > 0, 0 ≤ e < 1` is elliptic; `a_km < 0, e > 1` is hyperbolic (3I/ATLAS renders as an open arc per Rev B §9); `e = 1` (parabolic) is **rejected** — schema v1 has no consumer for it and the WP2 solver does not implement it. `m0_deg` is the mean anomaly at epoch; on the hyperbolic branch it is the hyperbolic mean anomaly `M = n·(t − tp)` with `n = √(μ/|a|³)`.

Mean motion at runtime: `mean_motion_deg_per_day` if present, else `√(μ_parent/|a|³)` — which is why validation requires a GM on every body that has children. Orbit-emphasis thresholds (Rev B §5) derive from the period at catalog load using exactly this rule.

Provenance: Rev B asked for a per-body source *comment*; this schema promotes it to a required `source` **field** instead — same audit value, but machine-checkable (validation fails on empty) and it survives serialization round-trips. The file header comment carries the regeneration command, frame, units, and timestamp.

## 4. Source routing and normalization

One route per body, declared in the manifest and printed by `--dry-run`:

**Sun — `SunFixed`.** No fetch; IAU nominal radius and GM from the manifest.

**Planets — `HorizonsPlanet`.** Horizons `MAKE_EPHEM`, `EPHEM_TYPE='ELEMENTS'`, `CENTER='500@10'`, `REF_PLANE='ECLIPTIC'`, `REF_SYSTEM='J2000'`, `OUT_UNITS='KM-S'`, `TLIST` in JD TDB. Mercury–Mars use geometric planet-center targets `199` / `299` / `399` / `499`; Jupiter–Neptune use planetary-system barycenter targets `5` / `6` / `7` / `8`, whose ephemerides cover the full fitting span. One request per planet has 13 epochs: the catalog epoch, epoch+1 day, and Jan-1 of 1800…2300 in 50-year steps. From these the generator fits, entirely from data (no hand-embedded Standish table to mistype):
- *Base elements:* the record at the catalog epoch (must be within 0.5 d).
- *Mean motion:* linear slope of unwrapped MA across the near-epoch pair — this captures the perturbation-averaged rate the way Standish's mean-longitude rates do, and it is why the runtime prefers the override to `√(μ/a³)`.
- *Secular rates:* least-squares linear fit of a, e, i, Ω, ω (angles unwrapped) over the 1800–2300 coarse samples, matching the Rev B soft time range. Spans under 50 years yield `secular: None` rather than a garbage fit.

**Moons — `HorizonsMoon`.** Same ELEMENTS request, `CENTER='500@<parent body center>'` (e.g. `500@599`), single sample at the catalog epoch, emitted parent-centric with no secular terms. This realizes Rev B §4.3's "parent-frame elements re-expressed in ecliptic at generation time" directly: Horizons does the frame work; the generator stores what comes back.

**TNO moons (Dysnomia, Hiʻiaka, Namaka) — `HorizonsLookupMoon`.** Their Horizons COMMAND codes and parent center designators are not stable well-known constants, so online generation queries the parent system through the Horizons Lookup API (`group=mb`). It requires API version 1.1, selects exactly one `asteroidal system primary` plus exactly one normalized-name satellite match, and uses their returned SPK IDs for the parent-centric ELEMENTS request. Missing or ambiguous matches fail loudly. Lookup payloads are captured as `<body-id>.lookup.json`; in `--fixtures` mode the resolved `<body-id>.json` ELEMENTS response is accepted directly.

**Dwarf planets, asteroids, comets — `Sbdb`.** `sbdb.api?sstr=<designation>&full-prec=true`. SBDB elements are already heliocentric ecliptic-J2000 with JD-TDB epochs; the only unit conversion is AU→km (IAU value 149,597,870.7). Two normalization rules:
- `a_km` from `a` when present, else `q/(1−e)` — automatically negative for `e > 1`, matching the schema convention with no special-casing.
- Mean anomaly from `ma` at the SBDB epoch when present (typical for asteroids); otherwise **re-base the epoch to perihelion**: `epoch := Tp`, `m0 := 0`. Exact by definition, and numerically the cleanest anchor for high-e (Halley) and hyperbolic (3I/ATLAS) orbits.

## 5. Time scales and frames — the silent-error watchlist

The V1 roadmap's §1.4 warning about silent numerical error applies at prototype scale too. The binding rules: every epoch in this pipeline is **TDB** (Horizons TLIST is requested as JD TDB; SBDB epochs and Tp are JD TDB) — UTC never enters the data path, only the cosmetic `generated_utc` stamp. Every request pins `REF_PLANE='ECLIPTIC'`, `REF_SYSTEM='J2000'`; equatorial frames never enter. `OUT_UNITS='KM-S'` keeps Horizons in file units; AU appears only inside the SBDB adapter. Any future change to these constants is a schema-review event, not a refactor.

## 6. Validation gates (in `sim-core`, shared by generator and app)

The generator **refuses to emit** any catalog that fails `Catalog::validate()`, and the app runs the same function at load — the two can never disagree about what a valid file is. Hard errors (all collected, not first-fail): schema version / frame mismatch; empty, malformed, or duplicate ids; case-insensitive search-key collisions across name+designation+aliases (WP12 search correctness depends on this — "3I/ATLAS" and "C/2025 N1" must both resolve, uniquely); star topology (exactly one star, no parent/orbit on it, orbit+parent required on everything else); unknown parents and non-terminating parent chains; heliocentric categories parented to anything but the star; moons parented to the star; parents without GM; non-finite anything; `e`/`sign(a)` mismatches; parabolic elements; epochs outside the JD sanity window; non-positive fitted mean motion; empty `source`. Soft lints (reported, non-blocking): empty descriptions (WP10 content debt) and untextured star/planets (WP15).

The corrupt-fixture half of the WP3 acceptance check ("loader rejects corrupt fixtures") is covered by unit tests today: truncated RON, wrong root type, NaN injection, duplicate/orphan/mis-parented records all fail loudly.

## 7. CLI and determinism

```
cargo run -p xtask -- gen-catalog --dry-run                 # print the fetch plan, no network
cargo run -p xtask -- gen-catalog --fixtures DIR [--allow-partial] [--out PATH] [--epoch-jd F]
cargo run -p xtask --features online -- gen-catalog --online [--capture DIR] [--out PATH] [--epoch-jd F]
```

Default epoch is JD 2461042.0 = 2026-01-01 12:00 TDB (Rev B's startup epoch). `--fixtures` runs the identical parse/normalize/validate/emit path against captured API responses; `--allow-partial` skips bodies without fixtures and prunes any resulting orphans, which is what the CI smoke test uses. In online mode, `--capture DIR` writes every exact raw response as `DIR/<body-id>.json` (plus `<body-id>.lookup.json` for TNO-moon resolution); parse failures also preserve the payload at `target/xtask-debug/<body-id>.response.txt` and report that path. Given fixed inputs the pipeline is deterministic except for the `generated_utc` stamp and regeneration-command header. The 2026-07 capture contains 68 payloads for all 66 bodies (the Sun needs no fetch; the three lookup moons each add a lookup payload), and fixture replay reproduces all catalog data exactly.

## 8. Open items (tracked, not forgotten)

1. **TNO satellite resolution — implemented.** The online path resolves the satellite and parent-primary SPK IDs from a strict parent-system lookup; offline fixtures remain direct ELEMENTS captures.
2. **Curated-value review — radius/TNO decisions complete.** Q2/Q3 were human-resolved on 2026-07-13: Pluto uses the 975.5 km³/s² Pluto+Charon system GM, Eris/Haumea retain their system GMs, and 3I/ATLAS uses an adopted 0.5 km radius with its observational range cited. The human also approved all eight flagged central-value changes in the all-66-body audit at `docs/wp3-radius-audit-2026-07-13.md`; each changed body carries its individual physical provenance and the radius review marker is cleared. The pre-existing general Sun/planet GM `TODO(review)` remains open and must be resolved separately before the TASKS.md curated-review checkbox can be checked.
3. **Descriptions** — ~50 bodies have empty blurbs (deliberate; WP10 content pass). The lint keeps the list visible.
4. **Textures** — `texture` is emitted as `None` everywhere until WP15; the license/source-per-asset CI check from Rev B §2 attaches there.

## 9. Interaction with WP2 and the acceptance check

WP3's acceptance ("10-body Horizons spot-check fixtures within two-body tolerance") compares **propagated positions**, which requires the WP2 Kepler solvers. Sequencing: capture, now, per spot-check body, Horizons **VECTORS** (not ELEMENTS) at two epochs — 2026-01-01 and 1986-02-09 (Halley perihelion, already a demo-script stop) — into `xtask/fixtures/spotcheck/`; when WP2 lands, a `sim-core` integration test propagates each body from its catalog elements to both epochs and asserts against those vectors within a documented two-body tolerance per category (planets tightest; comets loosest, dominated by ignored non-gravitational forces per Rev B §4.3). Suggested spot-check set, exercising every regime: Mercury (fast), Earth (reference), Jupiter (secular-sensitive), Sedna (extreme a), Io (fast moon), Triton and Phoebe (retrograde), Nereid (high-e moon), Halley (high-e comet), 3I/ATLAS (hyperbolic).

## 10. What downstream WPs may rely on

WP4 (propagation): elements + epoch + GM chain are sufficient for f64 heliocentric assembly (moon states compose parent-centric onto the parent's heliocentric state). WP9/WP12 (labels, search): `find()` semantics — case-insensitive exact match over name/designation/aliases — are the contract the fuzzy layer must preserve as a subset. WP10 (Info tab): `category`, `radius_km`, `parent`, `description`, and period via `Orbit::period_s(parent GM)`. WP6 (orbit lines): per-category `color_srgb`, and hyperbolic detection via `Elements::is_hyperbolic()` for the ±25-year open-arc sampling rule.
