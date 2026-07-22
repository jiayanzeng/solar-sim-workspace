# Solar System Simulator — Architecture Specification, Revision D

**NORMATIVE · HUMAN-CHANGE-CONTROLLED.** This file is the design of record.
AI coding agents MUST treat it as **read-only**: propose changes as an entry
in `TASKS.md → Open questions`, never as an edit here. Only the human
maintainer revises this document. On any conflict, precedence is:
**this file → `TASKS.md` → `AGENTS.md` → code comments**.

Revision D supersedes Rev C in full. It is self-contained — no other design
document is required to execute it — and it freezes the contracts that
WP1–WP3 turned from plan into shipped, tested code. Sections marked
**[AS BUILT]** describe code that exists and passes CI; changing those
contracts is an architecture revision, not a refactor. Sections marked
**[TO BUILD]** are prescriptive for future work packages.

Keywords MUST / MUST NOT / SHOULD / MAY are used in the RFC-2119 sense.

---

## 1. Product definition

A **Steam-releasable desktop solar-system simulator** modeled on the
interaction patterns and information architecture of NASA's *Eyes on the
Solar System*, with our own visual identity. Engine: **Bevy 0.19.x**
(pinned; see §8.1). Platforms: macOS first (universal, signed + notarized),
Windows 10+ second; Linux/Steam Deck is an unverified stretch. Scope: a
curated **66-body catalog** (§6), Keplerian two-body motion on both conic
branches, a time-rate ladder to **±100 years/second**, a configurable start
epoch defaulting to **2026-01-01 12:00 TDB**, and an Eyes-modeled UI at
Steam quality. Out of scope for this product (owned by the separate V1
roadmap): numerical N-body, real ephemerides, the 2050 regime boundary,
spacecraft, stories, mobile, web, HD streaming, anti-tamper.

**Legal boundary.** The interface copies Eyes' *interaction patterns*; the
name, logo, fonts, and styling are our own. The NASA insignia and name MUST
NOT appear in branding. Public-domain NASA/USGS imagery MAY be used as
textures with source attribution in credits and no implication of
endorsement. Every shipped asset and dataset MUST carry license/source
metadata, checked in CI.

## 2. Workspace layout

```
crates/sim-core/        pure-Rust engine core (catalog, time, kepler)  [AS BUILT]
crates/solar-sim/       the Bevy 0.19 application                      [TO BUILD, WP0]
xtask/                  offline dev tooling (gen-catalog, packaging)   [AS BUILT: gen-catalog]
  fixtures/             captured/synthetic API responses (labeled)
  fixtures/spotcheck/   Horizons truth captures (activates §5.6 gate)
assets/                 committed generated data (catalog.ron), textures, fonts
docs/                   per-WP specifications (e.g. wp3-gen-catalog-spec.md)
ARCHITECTURE.md         this file (read-only for agents)
TASKS.md                living status board (agents update per its protocol)
AGENTS.md               agent behavior constraints (root + nested)
```

Dependency firewall: `sim-core` MUST have **zero Bevy dependencies** and
compiles with `serde` + `ron` only. `xtask` MAY depend on `sim-core`, never
the reverse. `crates/solar-sim` depends on `sim-core` and Bevy. A CI check
(WP0) enforces core purity mechanically.

## 3. Non-negotiable invariants

These hold across every work package. Violating any of them is a defect
regardless of tests passing.

1. **Core purity.** `sim-core`: no Bevy, no network, no filesystem access
   (it parses strings handed to it), no system clock (callers pass wall
   time), no panics on untrusted input — `Result` with collected errors.
2. **Offline by default.** The shipped app never touches the network. Only
   `xtask` built with `--features online` may, at development time.
3. **Generated data is never hand-edited.** `assets/catalog.ron` is emitted
   by `xtask gen-catalog` and committed with a provenance header. The only
   hand-authored data is the curated manifest (§5.2), which is human-reviewed.
4. **One command path.** Every user action — keyboard, pointer, every widget
   — flows through a single `SimCommand` queue. No UI element mutates
   simulation state directly. This is what makes replay-based determinism
   testing possible, and replay determinism is a release gate.
5. **Units and frames are fixed.** All persisted and inter-module data:
   ecliptic-J2000 frame, kilometers, degrees in files / radians in math,
   Julian Date **TDB** for epochs, seconds-since-J2000 (`t: f64`, TDB) for
   runtime time. UTC exists only at the wall-clock boundary (§4.2). AU
   appears only inside the SBDB adapter.
6. **f64 truth, f32 render.** Simulation state is f64 heliocentric km.
   Rendering rebases to f32 `Transform`s around a floating origin at the
   camera focus (§8.3). Orbit-line vertices are parent-relative.
7. **Determinism.** Given identical command streams and fixed inputs, state
   hashes MUST be identical across platforms. Anything nondeterministic
   (wall clock, GPU timing) enters only through explicit parameters.
8. **Transition events, not level events.** Anything that triggers a toast
   or banner reports state *transitions* exactly once (as built in
   `SimClock::tick`), never per-frame levels.
9. **Validation symmetry.** The generator refuses to emit any file the
   app's loader would reject: both call the same `Catalog::validate()`.

## 4. `sim-core` — frozen contracts **[AS BUILT]**

The signatures below are stable API. Additive change is allowed; breaking
change requires revising this document.

### 4.1 `catalog` (schema v1)

Types: `Catalog { schema_version: u32 = 1, generated_utc, frame:
"ECLIPJ2000", bodies: Vec<BodyRecord> }`;
`BodyRecord { id, name, designation?, aliases, category, parent?,
gm_km3_s2?, radius_km, color_srgb: (u8,u8,u8), texture?, description,
orbit?, source }`;
`Orbit { epoch_jd_tdb, elements, secular: Option<SecularRates>,
mean_motion_deg_per_day: Option<f64> }`;
`Elements { a_km, e, i_deg, raan_deg, argp_deg, m0_deg }`.

Semantics and rules:

- `id` is the stable machine key (`[a-z0-9_]+`); commands, settings, and
  replay streams reference ids, never names.
- Conic convention: elliptic `a>0, 0≤e<1`; hyperbolic `a<0, e>1`; parabolic
  (`|e−1| < 1e-9`) is **rejected** everywhere.
- Comet orbits are epoch-re-based to perihelion at generation time:
  `epoch := Tp`, `m0 := 0`.
- `secular` (per-Julian-century linear rates on a/e/i/Ω/ω) is emitted for
  planets only, **fitted from Horizons sampling across 1800–2300**, never
  hand-typed. `mean_motion_deg_per_day` is a fitted override; when present
  it takes precedence over `√(μ/|a|³)` (`Orbit::mean_motion_rad_per_s`).
- `source` is a **required non-empty field** per body (machine-checkable
  provenance; Rev B's "source comment" is superseded).
- `Catalog::validate()` collects **all** errors: schema/frame mismatch, id
  charset, duplicates, case-insensitive search-key collisions across
  name+designation+aliases, exactly-one-star topology, unknown parents and
  parent cycles, category/parent legality, parents-with-children missing
  GM, non-finite values, e/sign(a) mismatch, parabolic, epoch outside the
  JD sanity window (2.2e6–2.6e6), non-positive fitted mean motion, empty
  source. `lint()` reports non-blocking debt (empty descriptions → WP10;
  untextured star/planets → WP15).
- `Catalog::find()` is case-insensitive exact match over
  name/designation/aliases; the WP12 fuzzy layer MUST preserve it as a
  subset ("3I/ATLAS" and "C/2025 N1" both resolve, uniquely).
- Constants: `AU_KM = 149_597_870.7`, `J2000_JD_TDB = 2_451_545.0`,
  `SECONDS_PER_DAY = 86_400`, `DAYS_PER_JULIAN_CENTURY = 36_525`.

### 4.2 `time`

`SimClock` holds `t: f64` (TDB seconds since J2000), a signed `RateIndex`,
`playing`, and snap state. **The caller supplies the wall clock**:
`tick(wall_dt_s, wall_now_t)` and `is_live(wall_now_t)` — the core never
reads the system clock (invariant 7).

- **Rate ladder.** `RateIndex` is a signed step `±1..±12`, **no zero**
  ("stopped" is `playing = false`, keeping the slider's center detent
  unambiguous). Magnitudes: REAL, 1 MIN/S, 1 HR/S, 1 DAY/S, 1 WK/S,
  1 MTH/S, 6 MTHS/S, 1 YR/S, 3 YRS/S, 10 YRS/S, 30 YRS/S, 100 YRS/S —
  months are **mean months** (Julian year / 12). Labels follow the Eyes
  convention (`"REAL RATE"`, `"6 MTHS/S"`, `"−3 YRS/S"`). 24 detents;
  `slider_pos()`/`from_slider_pos()` implement the symmetric-log mapping
  (uniform detent spacing); `stepped()` skips zero (+REAL → −REAL is one
  step) and saturates at ±12.
- **Range.** Soft range 1800-01-01T00:00 … 2300-12-31T23:59:59 TDB
  (`T_MIN_S = −6_311_390_400`, `T_MAX_S = 9_498_599_999`). Clamping
  **pins** (does not pause) — reversing the rate walks straight back off
  the edge. High-confidence boundary `T_HIGH_CONFIDENCE_MAX_S`
  (2051-01-01): outside 1800–2050, positions are "extrapolated" and the UI
  shows the §9.6 toast. Both the clamp and the boundary report
  **transitions only** via `TickReport`.
- **LIVE.** `is_live` ⇔ playing ∧ not snapping ∧ rate = +REAL ∧
  `|t − now| < 2 s`. `snap_to_live()` starts an exponential ease
  (τ = 0.12 s) toward the *moving* live target; landing sets +REAL/playing
  and reports `snapped_live` once.
- **Start epoch.** `StartMode::FixedEpoch { jd_tdb }` (default JD
  2_461_042.0 = 2026-01-01 12:00 TDB) or `StartMode::Live`; serde-derived
  for the WP14 settings framework.
- **Calendar.** Proleptic-Gregorian ↔ `t` via Hinnant's civil algorithms on
  **exact integer-second paths** (`SECONDS_J2000_MINUS_UNIX =
  946_728_000.0` — note the noon `.5` day; a fractional-JD detour loses
  ~10 µs and breaks second-exact display; this bug was hit and fixed once
  already, do not reintroduce it). Strict parsers `parse_date`
  (`YYYY-MM-DD`) and `parse_time` (`HH:MM[:SS]`); invalid input MUST leave
  the clock untouched. `format_date_eyes` → `"JUL 11, 2026"`.
- **Time-scale stance.** Wall clock arrives as Unix/UTC and converts via a
  constant TT−UTC = 69.184 s (`t_from_unix_utc`), documented as a
  visual-grade approximation (ignores TDB−TT periodic terms and future
  leap seconds).

### 4.3 `kepler`

- `solve_elliptic(M, e) -> E` and `solve_hyperbolic(M, e) -> H`: Newton
  with a **guaranteed bracketed-bisection fallback** (both equations are
  strictly monotone; brackets `[M−e, M+e]` and
  `[asinh(M/e), M/(e−1)]`). `NoConvergence` is a hard should-never-happen
  guard. Elliptic wraps M to (−π, π] preserving whole turns; hyperbolic M
  is unbounded and solved by odd-symmetry mirror.
- `state_at(orbit, μ, t_s) -> StateVector { position_km, velocity_km_s }`
  in the **parent's** ecliptic-J2000 frame — the one call WP4 makes per
  body per frame. Pipeline: secular drift applied about the element epoch
  (`elements_at`) → M from `m0 + n·Δt` with n from the fitted override or
  `√(μ/|a|³)` → perifocal state → standard 3-1-3 (Ω, i, ω) rotation.
  Retrograde needs no special case (pinned by Triton/Phoebe tests).
- **Velocity-consistency rule.** Velocities are derived from the *same* n
  that advances M (`v ∝ n·a²`), so velocity is exactly d(position)/dt even
  when a fitted override decouples n from `√(μ/|a|³)`. A central-difference
  test enforces this; breaking it silently corrupts orbit-line tangents.
- Guards: non-finite, μ ≤ 0, e < 0 or parabolic, sign(a)/e mismatch.
- Free helpers `dot/cross/norm` on `[f64; 3]` are public; WP4 uses them.
- Closed-form evaluation is **O(1) in time** — this is the architectural
  justification for the ±100 yr/s ladder and why no prefetch machinery
  exists in this codebase.

## 5. Data pipeline — `xtask gen-catalog` **[AS BUILT]**

Normative detail lives in `docs/wp3-gen-catalog-spec.md`; the binding
summary:

### 5.1 Modes
`--dry-run` (print fetch plan, no network) · `--fixtures DIR
[--allow-partial]` (offline; identical pipeline against captured responses;
partial mode skips missing bodies and prunes orphans) · `--online`
(feature-gated behind cargo feature `online`; the only network path in the
repo). Default epoch JD 2_461_042.0; `--epoch-jd` overrides.

### 5.2 Curated vs. generated (the anti-typo split)
Curated in `xtask/src/manifest.rs`, human-reviewed: id, name, designation,
aliases, category, parent, radius, GM (parents only), color, blurb, route,
provenance note. Generated from JPL, never hand-typed: **every orbital
element, epoch, secular rate, and fitted mean motion.** The manifest's own
tests pin 66 bodies, category counts 1/8/9/8/32/8, unique ids,
parents-precede-children, every-parent-has-GM. `TODO(review)` markers on
radii and the Pluto/Eris/Haumea GMs MUST be cleared for WP3 sign-off.

### 5.3 Routes
Sun: constants, no fetch.

Planets: Horizons ELEMENTS, `CENTER='500@10'`. Mercury–Mars target
geometric planet centers (`199` / `299` / `399` / `499`), while
Jupiter–Neptune target planetary-system barycenters (`5` / `6` / `7` /
`8`) for stable coverage across the full fit range. Each planet uses a
13-epoch TLIST: the catalog epoch, epoch+1 d, and Jan-1 of 1800…2300 in
50-year steps. Base elements come from the catalog-epoch record. Fitted
mean motion is the linear slope of unwrapped mean anomaly across the
near-epoch pair. Secular rates for a/e/i/Ω/ω are least-squares fits over
the coarse span; spans shorter than 50 years yield `None`.

Moons: Horizons ELEMENTS, `CENTER='500@<parent>'`, single sample at the
catalog epoch, parent-centric, with no secular terms. Dysnomia, Hiʻiaka,
and Namaka resolve their satellite and parent-primary SPK IDs through
the Horizons Lookup API at generation time; offline fixtures contain
the resolved ELEMENTS response directly.

Dwarf planets, asteroids, and comets: SBDB with `full-prec`. SBDB’s AU
distances are converted to km here—the only AU→km conversion in the
repository. Use `a` when present; otherwise derive `a = q/(1−e)`, which
automatically produces negative `a` for hyperbolic `e > 1`. Use mean
anomaly at the SBDB epoch when present; otherwise re-base to perihelion
with `epoch := Tp` and `m0 := 0`.

### 5.4 Request pinning
Every Horizons request pins `EPHEM_TYPE='ELEMENTS'`,
`REF_PLANE='ECLIPTIC'`, `REF_SYSTEM='J2000'`, `OUT_UNITS='KM-S'`, TLIST in
JD TDB. Changing any of these constants is a schema-review event.

### 5.5 Emission
Provenance header (regeneration command, frame, units, timestamp) +
pretty RON; `Catalog::validate()` runs before write (invariant 9).

### 5.6 Position spot-check gate **[armed, awaiting data]**
`xtask/tests/spotcheck.rs` is dormant until the online run drops
`fixtures/spotcheck/catalog.ron` + `vectors.json` (≥10 bodies of Horizons
VECTORS truth, **parent-centric**, ecliptic-J2000:
`[{id, jd_tdb, position_km, tol_km}]`). Suggested set exercising every
regime: Mercury, Earth, Jupiter, Sedna, Io, Triton, Phoebe, Nereid,
Halley, 3I/ATLAS; epochs 2026-01-01 and 1986-02-09. Tolerances are
per-category two-body budgets (planets tightest; comets loosest — ignored
non-gravitational forces dominate).

## 6. Body catalog composition (66)

| Category | Count | Members |
|---|---|---|
| Star | 1 | Sun |
| Planets | 8 | Mercury … Neptune |
| Dwarf planets | 9 | Ceres; Pluto; Eris, Haumea, Makemake, Gonggong, Quaoar, Orcus, Sedna |
| Asteroids | 8 | 2 Pallas, 3 Juno, 4 Vesta, 10 Hygiea, 16 Psyche, 433 Eros, 101955 Bennu, 99942 Apophis |
| Moons | 32 | Earth: Moon · Mars: Phobos, Deimos · Jupiter: Io, Europa, Ganymede, Callisto, Amalthea, Himalia · Saturn: Mimas, Enceladus, Tethys, Dione, Rhea, Titan, Hyperion, Iapetus, Phoebe · Uranus: Miranda, Ariel, Umbriel, Titania, Oberon · Neptune: Triton, Proteus, Nereid · Pluto: Charon, Nix, Hydra · Eris: Dysnomia · Haumea: Hiʻiaka, Namaka |
| Comets | 8 | 1P/Halley, 2P/Encke, 9P/Tempel 1, 67P/Churyumov–Gerasimenko, 103P/Hartley 2, C/1995 O1 (Hale–Bopp), C/2020 F3 (NEOWISE), 3I/ATLAS (C/2025 N1, hyperbolic) |

Declared simplifications (visible only to experts at this accuracy):
two-body motion about the parent's center (no barycenters; Charon circles
Pluto); moon elements re-expressed in ecliptic at generation time; comet
non-gravitational forces ignored; Haumea rendered spherical (ellipsoid mesh
is a stretch). Growing the catalog later is content work, not engineering —
but **changing the 66-body composition requires human sign-off** (the
manifest count tests exist to make silent drift impossible).

## 7. Time-system semantics (UI-facing) **[TO BUILD on §4.2]**

The WP8 time bar binds directly to `SimClock`: date ("JUL 11, 2026"), rate
label, and clock as click-to-edit text (strict parse, revert-on-invalid);
play/pause; the detented symmetric-log slider (drag emits
`SimCommand::SetRate` — same path as keyboard); the LIVE chip (green dot +
text when `is_live`, dimmed pill button otherwise; click →
`snap_to_live`). Toasts consume `TickReport` transitions: range clamp at
1800/2300, extrapolation notice outside 1800–2050, and orbit-emphasis
onset (§10.4).

**Temporal aliasing is handled honestly, not hidden.** At 100 yr/s and
60 fps one frame spans ~1.7 years (~7 Mercury orbits). Per body, compute
the parent-relative phase step per frame; above **~0.15 rad**, cross-fade
the body dot and label out while brightening its orbit line, restoring
them as the rate drops (orbit-emphasis mode). Thresholds derive from
period at catalog load via `Orbit::period_s`. At 100 yr/s the inner system
correctly reads as a diagram of glowing orbits while outer dwarfs and
comets still crawl.

A settings-owned startup rate (factory default +1 day/s) is applied at
session start through one recorded SetRate command. StartMode::Live seeding,
the LIVE predicate, and LIVE snap remain exactly +REAL; a default boot
therefore starts near, but not at, LIVE.

## 8. Bevy application **[TO BUILD]**

### 8.1 Engine pinning
**Bevy 0.19.x exactly** — patch upgrades allowed, minor upgrades forbidden
mid-project; `Cargo.lock` committed. WP0 pins the Rust toolchain via
`rust-toolchain.toml` to the minimum stable Bevy 0.19 supports (note:
`sim-core` as built compiles on rustc 1.75+; keep its MSRV conservative).

### 8.2 Plugin graph and frame flow
`crates/solar-sim` plugins: `TimePlugin → PropagationPlugin → OriginPlugin
→ CameraPlugin → LabelsPlugin`; plus `ScenePlugin` (bodies, Sun light,
bloom, starfield), `OrbitLinesPlugin`, `SelectionPlugin` (picking, travel),
`UiKit` (BSN widget library + theme), `HudPlugin` (top bar, breadcrumb,
left panel, layers panel, right rail, time bar, toasts),
`SearchMenuPlugin`, `SettingsUiPlugin`, `PlatformPlugin` (render recovery,
window state), `SteamPlugin` (cargo feature `steam`).

Frame flow: input & UI → `SimCommand` queue → `SimClock` → propagation
(f64 heliocentric km for all 66 bodies via `kepler::state_at`, moon states
composed parent-centric onto the parent's heliocentric state) → floating-
origin rebase to f32 `Transform`s (origin = camera focus) → camera
rig/tween → label projection + declutter → Bevy UI.

### 8.3 Precision at Kuiper Belt scale
Render mapping **1 unit = 1,000 km**. Sedna's aphelion (~1.4×10⁸ units) is
inside f32 range; at that distance the ~10-unit representable error is
sub-pixel, and the floating origin keeps near-focus math exact. Far plane
~1×10⁹ units under reversed-Z. Zoom clamp: 1.2× body radius …
~1.5× Sedna aphelion — the full-system "vastness" shot is a supported view.

### 8.4 UI stack
First-party **Bevy UI on 0.19**: BSN scenes, `EditableText` (search field,
click-to-edit date/clock), `LetterSpacing` for the wide-tracked uppercase
label style, AccessKit-backed accessibility (`AccessibleLabel` on every
widget). A custom `ui_kit` widget set (panel, tab bar, checkbox row,
section header, chip, slider, toast) is built as BSN scene functions with
our dark theme; individual Feathers widgets MAY be reskinned and reused.
**Fallback policy:** `ui_kit` call sites stay stable while internals may
fall back to classic spawn APIs; `bevy_egui` is documented break-glass for
whole panels. The label system deliberately uses plain UI nodes positioned
from projection math so it works identically under any fallback.

### 8.5 Product plumbing
0.19 `SettingsPlugin` (reverse-domain identifier) persists: display mode,
resolution, vsync/frame cap, quality preset, UI scale, units (km/mi/AU),
start epoch / start-live, invert axes, layer states. Render recovery:
`DeviceLost → Recover`, `OutOfMemory → StopRendering` with a user-facing
error screen. Not used, by decision: Solari, contact shadows, area lights,
`.bsn` asset files (all scenes are code-defined).

## 9. Eyes-modeled interface **[TO BUILD]**

Visual identity: near-black background, hairline separators, one accent
color for active states, an SIL-OFL font family (e.g. Inter), wide-tracked
uppercase for primary labels. This section is the polish contract —
pixel-perfection beyond it is out of scope for beta.

1. **Top bar.** Our logo + name; a breadcrumb that *is* the navigation
   stack ("Solar System › Jupiter › Moons"); search (EditableText; fuzzy,
   case-insensitive, alias-aware, instant dropdown, Enter travels to top
   hit); Menu → full-screen **browse page** with three category columns
   (Planets & Moons / Dwarf Planets & Asteroids / Comets), curated
   shortlists, live counts derived from the catalog at load, expandable
   full lists.
2. **Left panel** (contextual, collapsible, tabbed; tab set data-driven per
   body class). *Info*: name, category chip with colored dot, radius,
   orbital period, parent, curated description; collection rows ("Moons of
   Jupiter (6)") navigate to collection pages. *View Options*:
   exaggerate-body-sizes toggle (×1/×10/×50 visual only), per-system moon
   visibility (Major/All), local orbit-line toggle.
3. **Layers quick panel** (bottom-right, opens from the right rail).
   Groups with separators: User Interface · Planets, Dwarf Planets,
   Asteroids, Comets · Moons · Orbits, Labels, Icons. UI-off yields a clean
   presentation mode with a small restore affordance. State persists via
   settings. Moons (default on) gates contextual moon presentation: moon
   spheres and orbits render only for the focused system, subject to the
   per-system Major/All option; off hides all moons. The persisted key,
   replay slug, and panel row are unchanged from Rev C.
4. **Right rail.** Zoom +/−, fullscreen, settings.
5. **Time bar.** Per §7.
6. **Toasts.** Non-blocking, bottom-left, auto-dismiss (delayed commands):
   extrapolation notice, range clamp, orbit-emphasis onset ("Inner orbits
   shown as paths at this speed").

Four region presets — Inner, Belt, Outer, Kuiper — are semantic travel
commands (focus Sun, canonical pose, fixed framing distances of
1.8/3.6/35/55 AU), surfaced as keys 1–4, Help entries, and Menu rows.

## 10. Rendering **[TO BUILD]**

1. **Bodies.** UV spheres at true radius × optional visual exaggeration;
   StandardMaterial (Metal bindless fast path applies); emissive Sun +
   point light + bloom; low ambient for night-side legibility; translucent
   disc for Saturn's rings. 2K public-domain textures (NASA SVS/USGS),
   KTX2; every body renders with its catalog color untextured, so
   texturing is polish, not a dependency. Render-only minimum apparent size:
   every non-Sun sphere is scaled so its projected diameter is at least 3
   logical px, applied after the optional ×10/×50 exaggeration; physical
   truth, picking, and orbits are unaffected. Comet tails are a specified
   post-beta fast-follow (see the 2026-07-22 plan, R3c).
2. **Orbit paths.** Ellipses sampled adaptively (denser near perihelion,
   256–768 points by eccentricity) in the **parent frame**; per-category
   color LUT (planets individually colored); distance/angle alpha fade.
   Hyperbolic (3I/ATLAS): open arc sampled over **±25 years around
   perihelion** (`Elements::is_hyperbolic` selects the branch). Secular paths
   may reuse retained geometry under a conservative sub-quarter-pixel
   screen-space drift bound; non-secular paths reuse exactly.
3. **Labels, icons, picking.** Labels are Bevy UI nodes repositioned each
   frame from `world_to_viewport`: wide-tracked uppercase for Sun +
   planets; small mixed-case beside a circular reticle (the Icons layer)
   for everything else. Declutter priority ladder: **selection › planets ›
   dwarf planets › comets › moons of the focused system › asteroids ›
   other moons**, greedy screen-rect rejection; out-of-system moons
   label-hidden beyond a parent-distance threshold. Labels are click
   targets; 3D picking backs them with ray-vs-inflated-bounding-sphere.
   Selection triggers the eased travel tween; the camera parents to the
   moving focus, so Follow is emergent.
4. **High-rate behavior.** Orbit-emphasis per §7 — a render-side rule with
   no data dependencies.
5. **Starfield.** ~5,000 Yale Bright Star Catalog stars baked at build time
   into a point mesh on the celestial sphere (equatorial→ecliptic tilt),
   magnitude-scaled sizes; optional faint Milky Way band. Constellation
   figures are a fast-follow pending a license-clean line set.

## 11. Steam release engineering **[TO BUILD]**

Feature-gated `SteamPlugin` wrapping `steamworks`: init with App ID,
shutdown on exit, nothing else at first — all Steamworks calls behind a
small `PlatformServices` trait. The **overlay on Metal** is the top
integration risk: dedicated spike early in WP16 on both OSes; the app MUST
NOT require the overlay. Packaging: macOS universal (`lipo`), `.app`
bundle, Developer ID signing + notarization (dry-run the full
sign/notarize/staple flow in WP16, not at ship); Windows signed exe +
assets; SteamPipe depots (`macos`, `windows-x64`) with `dev → beta →
default` branches, scripts in `xtask`. Performance gates: 60 fps on an M1
MacBook Air and a GTX 1650-class laptop with all layers on. Bundle
≤ 150 MB/platform. Licensing audit (fonts, textures, star data, no NASA
branding) is a release gate.

## 12. Testing standards

As practiced in WP1–WP3 and binding henceforth:

- **Full suite green** (`cargo test`, workspace) before any task is called
  done; new code lands with tests in the same change.
- **Physics/math code** requires (a) invariant tests (energy, angular
  momentum, closure, symmetry — constant to ~1e-10 where applicable) and
  (b) an **independent cross-validation** where feasible (the RK4
  vs closed-form pattern; the dual-source instinct at unit scale).
- **Solvers** ship with convergence sweeps over the full supported
  parameter domain, including the extreme inputs the product actually
  generates (huge M at ±100 yr/s).
- **Event emitters** ship with transition-only tests (exactly one event
  per crossing, zero per-frame spam).
- **Data code** ships with corrupt-input rejection tests.
- From WP5 on: **input-replay determinism** (identical state hashes across
  OSes) asserted in CI. From WP15: **golden screenshots** per backend for
  six canonical views.
- CI (WP0): fmt, clippy, nextest, macOS + Windows builds, core-purity
  rule, offline-by-default (no `online` feature in CI builds).

## 13. Definition of Done (Steam beta)

Fresh checkout builds with one cargo command per platform; Steam
beta-branch install launches clean on stock reference hardware; the app
opens on the configured 2026 epoch with all 66 bodies placed plausibly;
the demo script passes end-to-end (2026 start → search "Sedna" → travel
out to the full-system view → Menu browse to Jupiter → moons + View
Options → scrub to Halley's 1986 perihelion at −3 yr/s → push to
+100 yr/s and watch orbit-emphasis engage → LIVE snap); 60 fps holds with
all layers on reference hardware; settings persist; simulated device-loss
recovers; `sim-core` tests, the spot-check gate, and the replay
determinism suite pass on both OSes; the licensing audit is signed off;
bundle ≤ 150 MB/platform.

## 14. Risk register (watch list, priority order)

BSN/Bevy-UI rough edges (mitigation: stable `ui_kit` façade, classic-spawn
fallback, `bevy_egui` break-glass) · Steam overlay on Metal (early spike;
never required) · Bevy 0.19 youth (pin minor, absorb patches) · catalog
data errors (generated file + armed spot-check gate + curated review
markers) · UI polish absorbing unbounded time (§9 is the contract) ·
declutter quality at 66 bodies (tiered priority + contextual gating, tuned
with saved camera scenes in WP9) · macOS notarization friction (dry-run in
WP16).

## 15. Constants quick reference

| Constant | Value | Where |
|---|---|---|
| J2000 epoch | JD 2 451 545.0 TDB | `catalog::J2000_JD_TDB` |
| Default start epoch | JD 2 461 042.0 = 2026-01-01 12:00 TDB | `time::DEFAULT_START_EPOCH_JD_TDB` |
| Soft range | −6 311 390 400 … 9 498 599 999 s | `time::T_MIN_S/T_MAX_S` |
| High-confidence max | 1 609 416 000 s (2051-01-01) | `time::T_HIGH_CONFIDENCE_MAX_S` |
| J2000 − Unix epoch | 946 728 000 s (noon!) | `time` (private, tested) |
| TT − UTC | 69.184 s | `time::TT_MINUS_UTC_S` |
| LIVE epsilon | 2 s | `time::LIVE_EPSILON_S` |
| AU | 149 597 870.7 km | `catalog::AU_KM` |
| Julian year | 31 557 600 s | `time::JULIAN_YEAR_S` |
| Parabolic rejection window | \|e−1\| < 1e-9 | `kepler` |
| Orbit-emphasis threshold | ~0.15 rad phase step/frame | §7 |
| Render scale | 1 unit = 1 000 km | §8.3 |
