# TASKS.md — Living Status Board

This is the **only** project file AI coding agents update. `ARCHITECTURE.md`
is read-only; `AGENTS.md` files are human-maintained.

## Update protocol (binding on agents)

Agents MAY: flip a task's status, check checklist items, append to the
**Change log**, and add entries under **Open questions**. Every status
change MUST cite evidence (test run, file, or command output) in the change
log entry.

Agents MUST NOT: add or remove work packages; edit acceptance criteria;
change the 66-body catalog composition; resolve an Open question themselves
(humans close them); mark anything ✅ with a failing or skipped test suite.

Statuses: `todo` · `in-progress` · `blocked(reason)` · `✅ done` ·
`deferred`. One WP `in-progress` per agent at a time; finish or hand back
before starting another.

**How to execute a WP brief.** Each brief below has *Goal / Read first /
Build / Out of scope / Acceptance / Tests required*. Read the cited
ARCHITECTURE sections before writing code — the brief is a pointer, the
architecture is the contract. Acceptance checkboxes are the definition of
done; agents check them with evidence but never reword them. Anything a
brief leaves ambiguous becomes an Open question, not an improvisation.

---

## Dashboard

| WP | Deliverable | Status |
|---|---|---|
| 0 | Workspace, Bevy 0.19 pin, CI, window+camera+diagnostics, core-purity rule | **✅ done** |
| 1 | `sim-core::time` — full ladder, start epoch, LIVE, range | **✅ done** |
| 2 | `sim-core::kepler` — elliptic + hyperbolic, guards | **✅ done** |
| 3 | `xtask gen-catalog` + committed 66-body `catalog.ron` + validation | **✅ done** |
| 4 | Propagation + floating origin: 66 colored spheres at 2026 positions | **✅ done** |
| 5 | Camera rig, input-intent layer, key map, travel tween, replay determinism | **✅ done** |
| 6 | Orbit lines (adaptive; hyperbolic arc), colors, fades | **✅ done** |
| 7 | `ui_kit`: theme, fonts, BSN widgets, top bar + breadcrumb | **✅ done** |
| 8 | Time bar: detented log slider, editable date/clock, LIVE chip | **✅ done** |
| 9 | Labels/reticles, tiered declutter, contextual moon visibility, picking | **✅ done** |
| 10 | Left panel: Info tab, collection pages, View Options | **✅ done** |
| 11 | Layers quick panel, right rail, Icons layer, UI-off mode | **✅ done** |
| 12 | Search (alias-aware) + Menu browse with live counts | **✅ done** |
| 13 | Orbit-emphasis high-rate mode; BSC starfield; Sun bloom | **✅ done** |
| 14 | Settings screen + render-recovery policies | **✅ done** |
| 15 | Texture pass (2K KTX2) + visual polish + golden screenshots | **✅ done** |
| 16 | Steam: Steamworks init, overlay spike, packaging/signing/depots | todo |
| 17 | QA: replay suite, perf gates, demo script, licensing audit | todo |
| 18 | *Optional:* Compare Size mode | deferred |

**Test baseline: 196 passing** (53 `sim-core` · 100 `solar-sim` · 40 `xtask`
lib · 2 xtask smoke · 1 spot-check gate, active). Any change that lowers
this number without an accompanying change-log justification is a regression.
The number may only go up.

---

## Done (evidence)

### WP1 — `sim-core::time` ✅
`crates/sim-core/src/time.rs` (22 tests). RateIndex ±1..±12 (no zero), 24
detents, Eyes labels, symmetric-log slider mapping; SimClock with
caller-supplied wall clock; range pins with transition-only `TickReport`;
eased snap-to-LIVE; exact-integer calendar with leap-rule round-trips;
strict date/time parsers; `StartMode` serde-ready for WP14.

### WP2 — `sim-core::kepler` ✅
`crates/sim-core/src/kepler.rs` (14 tests). Newton + guaranteed bisection
on both branches; convergence sweeps e∈{0…0.97}×720 M's + huge-M, and
e∈{1.2…6}×10 decades; `state_at` with secular application and
velocity-consistency under fitted mean motion (central-difference
enforced); invariants to 1e-10; RK4 cross-validation through perihelion on
both branches; retrograde (Triton i=157°, Phoebe i=175°) and Nereid
(e=0.75) fixtures; guard tests.

### WP3 (core) — schema + generator ✅
`crates/sim-core/src/catalog.rs` (16 tests), `xtask/*` (27 lib + 2 smoke).
Schema v1 with collect-all validation + lints; 66-body curated manifest
with count/order/GM tests; Horizons + SBDB parsers; fitted secular/mean-
motion normalization; `--dry-run` / `--fixtures --allow-partial` /
feature-gated `--online`; provenance-headed emission; offline smoke
produces `assets/catalog.sample.ron` (6 bodies) that reloads through the
app loader. Spec: `docs/wp3-gen-catalog-spec.md`.

### WP4 — propagation + floating origin ✅
`crates/solar-sim/src/lib.rs` loads and validates the committed catalog,
holds all 66 heliocentric states in f64, composes moons in one ordered forward
pass, and performs the only f64→f32 conversion in the 1,000 km/unit origin
rebase. True-radius colored sphere entities, an emissive Sun, +REAL
`SimClock`, command-routed scaffolding, user-facing catalog errors, and the
explicit input → commands → clock → propagation → origin → render schedule
are active.

---

## WP3 — acceptance complete

- [x] **Online capture run** (needs JPL network access; run
  `cargo run -p xtask --features online -- gen-catalog --online --out
  assets/catalog.ron`). Commit the emitted file *and* the captured API
  responses for reproducibility. Q5 is closed and its approved route split
  is implemented: Mercury–Mars target geometric centers, while
  Jupiter–Neptune target system barycenters.
  The 2026-07-13 live run succeeded for all 66 bodies and produced
  `assets/catalog.ron` plus 68 raw captures (65 body responses and three
  TNO lookup responses). Commit `1ea4d1f` records the generated catalog,
  all 68 captured responses, and the active spot-check catalog/vectors;
  `origin/main` contains that commit.
- [x] **TNO moon resolution**: Horizons lookup-API resolution for
  Dysnomia / Hiʻiaka / Namaka COMMANDs and Eris/Haumea center designators
  (`xtask/src/lookup.rs`; strict API-version and unique-match checks).
- [x] **Curated review pass**: clear every `TODO(review)` in
  `xtask/src/manifest.rs` (all radii; GMs for Pluto / Eris / Haumea;
  3I/ATLAS nucleus radius). Research brief with citations and
  recommendations: `docs/open-questions-brief-2026-07-12.md` (Q2, Q3).
  Human sign-off required; agents prepare the diff + citations only.
  Note the Pluto-GM semantics question raised there (Pluto-only 869.6 vs
  Pluto+Charon ≈ 975.5 for correct Charon period under μ=parent-GM
  propagation) — that is part of Q2's human decision.
  Completed 2026-07-13: Q2/Q3, all 66 radii, the TNO-system GMs, and the
  exact JPL DE440 Sun/planet GM set are human-approved and applied with
  per-body provenance. No `TODO(review)` remains in the manifest.
- [x] **Spot-check activation**: capture Horizons VECTORS for the 10-body
  set (ARCHITECTURE §5.6) at JD 2461042.0 and 1986-02-09 into
  `xtask/fixtures/spotcheck/vectors.json`; document per-category
  tolerances in `docs/wp3-gen-catalog-spec.md`; `cargo test` must show the
  gate passing (not skipping).
  Q6 was human-approved 2026-07-13: all 10 bodies gate at JD 2461042.0,
  Halley additionally gates at JD 2446471.0, and the other nine historical
  captures remain audit-only. The test pins this selection and the ordered
  1 km / 10 km / 25,000 km / 30,000,000 km category budgets, which the
  human explicitly approved on 2026-07-13.

## WP0 — acceptance complete

Human walkthrough completed from `docs/wp0-dev-setup-macos.md`.

- [x] `rust-toolchain.toml` pinning **1.95.0** — Bevy 0.19.0's declared
  MSRV per crates.io (Q1, closed). Keep `sim-core`'s own
  MSRV claim conservative (`rust-version = "1.75"` in its Cargo.toml).
- [x] `crates/solar-sim` skeleton: Bevy 0.19.x pinned in `Cargo.toml`
  (`bevy = "0.19"` + committed `Cargo.lock` = exact pin), window opens,
  orbit camera stub, dev-only `DiagnosticsOverlay`, `--smoke` flag
  (render N frames, exit 0) for CI launch checks.
- [x] CI (GitHub Actions): fmt, clippy (deny warnings), nextest,
  macOS + Windows build jobs, **core-purity rule** (fail if `sim-core`'s
  dependency tree contains any `bevy*` crate), offline rule (no `online`
  feature — and therefore no `ureq` — in default/CI builds).
- [x] Acceptance: app opens on macOS; Windows job compiles and links in
  CI; CI green.

Deferred pre-WP16 release gate (not part of WP0 acceptance):
- [ ] Windows launch verified on real hardware or a VM.

---

## Work package briefs (WP4–WP18)

Acceptance criteria below are **human-owned**: agents check boxes with
evidence, never edit the text. "Perf budget" means 60 fps on the WP17
reference hardware unless a brief says otherwise.

### WP4 — Propagation + floating origin

**Goal.** All 66 bodies exist as colored, true-radius spheres at their
2026-01-01 12:00 TDB positions, driven by `sim-core`, with f64 truth and a
floating origin so nothing jitters anywhere from Mercury to Sedna.

**Depends on.** WP0. Real `catalog.ron` for acceptance;
`catalog.sample.ron` is fine for development.

**Read first.** ARCHITECTURE §3 (invariants 5–7), §4.3 (`state_at`,
helpers), §8.2 (frame flow), §8.3 (precision, 1 unit = 1,000 km).

**Build.**
- Catalog load at startup through `sim-core`'s loader (the same
  `Catalog::validate()` path as the generator); a load failure is a
  user-facing error screen, not a panic.
- `PropagationPlugin`: per frame, for every body with an orbit, call
  `kepler::state_at(orbit, μ_parent, t)`; compose moon states
  parent-centric onto the parent's heliocentric state. Catalog order
  guarantees parents precede children, so one forward pass suffices.
  All state is f64 heliocentric km in a resource/components — never f32.
- `OriginPlugin`: rebase f64 heliocentric km → f32 `Transform` around the
  camera-focus origin at 1 unit = 1,000 km; runs after propagation,
  before rendering.
- Sphere meshes at true radius × 1.0 (exaggeration is WP10), per-body
  catalog `color_srgb`, Sun emissive placeholder (bloom is WP13).
- Time driven by `SimClock` at +REAL by default (full time bar is WP8;
  a debug key to step rates is acceptable scaffolding).

**Out of scope.** Orbit lines (WP6), labels/picking (WP9), size
exaggeration (WP10), bloom/starfield (WP13).

**Acceptance.**
- [x] With the real `catalog.ron`, heliocentric longitudes of the 8
  planets at the 2026 epoch match Horizons to eyeball accuracy (or, once
  the spot-check data exists, the WP4-side positions match `sim-core`'s
  spot-checked output bit-for-bit).
- [x] No visible jitter at closest zoom focused on Mercury; none focused
  on Sedna (the two precision extremes).
- [x] All 66 bodies render; frame flow order is input → commands → clock
  → propagation → origin → render (verified by system ordering, not luck).
- [x] Perf budget holds with all 66 bodies.

**Tests required.** A propagation unit test comparing the Bevy-side
composed state of at least one moon (e.g. Io) against a pure `sim-core`
reference computation — identical to the last bit; an origin-rebase test
(focus change leaves relative positions unchanged to f32 eps); the
load-failure path rejects a corrupt catalog without panicking.

### WP5 — Camera rig, input-intent layer, key map, travel tween, replay determinism

**Goal.** All interaction flows through `SimCommand`; the camera is an
orbit rig around a focus with an eased travel tween; the determinism suite
(record → replay → identical state hash) starts here and never leaves CI.

**Depends on.** WP4.

**Read first.** ARCHITECTURE §3 (invariants 4, 7), §8.2, §8.3 (zoom
clamps), §12 (replay determinism).

**Build.**
- Input-intent layer: raw keyboard/mouse → semantic intents → `SimCommand`
  variants (select, travel, orbit, dolly, set-rate, play/pause…). No
  system outside this layer reads raw input; no system outside the command
  consumer mutates sim state.
- Orbit rig: yaw/pitch/dolly about the focus body; zoom clamp 1.2× body
  radius … ~1.5× Sedna aphelion; camera parents to the moving focus so
  Follow is emergent.
- Travel tween: eased focus + framing transition to a *moving* target;
  interruptible by a new selection.
- Replay: serialize command streams with frame/time stamps; a replay
  harness feeds them to a headless app run and hashes sim state (f64 state
  only, never render state); identical inputs ⇒ identical hash.

**Out of scope.** Picking/selection UI (WP9) — WP5 may use a debug key to
change focus.

**Acceptance.**
- [x] Zero direct sim-state mutation outside the command consumer
  (enforced by module visibility or a grep-able convention documented in
  the code).
- [x] Travel tween to a moving target (e.g. Io) lands and follows without
  a snap.
- [x] Zoom clamps hold at both ends.
- [x] Replay determinism test runs in CI on macOS and Windows and produces
  identical hashes across both.

**Tests required.** Replay round-trip (record 500+ mixed commands, replay,
assert equal hash); tween convergence test; input-map table test (every
bound key produces exactly one command).

### WP6 — Orbit lines

**Goal.** Every orbiting body can show its path: adaptively sampled
ellipses in the parent frame, per-category/per-planet colors, distance and
angle fades, and the 3I/ATLAS hyperbolic arc over ±25 years around
perihelion.

**Depends on.** WP4.

**Read first.** ARCHITECTURE §3 (invariant 6: vertices parent-relative),
§10.2.

**Build.**
- Sampler: 256–768 vertices by eccentricity, denser near perihelion
  (uniform-in-anomaly is acceptable if the density criterion is met);
  vertices computed in the parent frame from the same elements the
  propagator uses (`elements_at` at current t so secular drift matches).
- Hyperbolic branch: open arc over ±25 yr around perihelion, branch
  selected via `Elements::is_hyperbolic`.
- Color LUT: per-category defaults, planets individually colored; alpha
  fade by camera distance and by viewing angle per §10.2.
- Lines re-anchor under the floating origin like bodies do.

**Out of scope.** The local orbit-line toggle UI (WP10), the layers panel
(WP11), orbit-emphasis brightening (WP13) — but leave a brightness input
hook for WP13.

**Acceptance.**
- [x] Ellipses close with no visible seam gap; Nereid (e=0.75) shows
  visibly denser sampling near perihelion.
- [x] 3I/ATLAS renders an open arc spanning ±25 yr around perihelion and
  never a closed loop.
- [x] No orbit-line jitter or z-fighting flicker at full-system zoom while
  focused on an inner body.
- [x] Perf budget holds with all orbit lines on.

**Tests required.** Sampler unit tests (vertex count bounds by e; first ==
last for elliptic; endpoints at ±25 yr for hyperbolic; all vertices finite);
a consistency test that sampled perihelion distance matches
`a(1−e)`/`|a|(e−1)` to tolerance.

### WP7 — `ui_kit`: theme, fonts, widgets, top bar + breadcrumb

**Goal.** The reusable widget layer every later UI package builds on: our
dark theme, an SIL-OFL font family, BSN scene-function widgets with
accessibility labels, and the first real HUD surface (top bar + breadcrumb).

**Depends on.** WP0 (WP4 useful but not required).

**Read first.** ARCHITECTURE §8.4 (UI stack + fallback policy), §9 intro
(visual identity), §9.1 (top bar).

**Build.**
- Theme resource: near-black background, hairline separators, one accent
  color, type scale; wide-tracked uppercase style via `LetterSpacing`.
- Font: pick and vendor an SIL-OFL family (e.g. Inter); record license
  metadata (WP17 audit input).
- Widgets as BSN scene functions: panel, tab bar, checkbox row, section
  header, chip, slider, toast. Every widget takes an `AccessibleLabel`.
- Call-site-stable API: internals may fall back to classic spawn without
  changing signatures (the §8.4 fallback policy is a design input, not a
  comment).
- Top bar: logo + product name; breadcrumb bound to the navigation stack
  ("Solar System › Jupiter › Moons"); search field placeholder (behavior
  lands in WP12).
- A `widget_gallery` dev scene (debug builds) rendering every widget in
  every state.

**Out of scope.** Search behavior (WP12), left panel (WP10), layers panel
(WP11), time bar (WP8).

**Acceptance.**
- [x] Widget gallery shows every widget in default/hover/active/disabled
  states with the theme applied.
- [x] Every widget carries an AccessKit label (verified via the gallery).
- [x] Breadcrumb reflects a scripted navigation-stack push/pop sequence.
- [x] Font license file vendored beside the font with source noted.

**Tests required.** Theme-token snapshot test (colors/spacing constants
don't drift silently); breadcrumb model unit test (push/pop/truncate).

### WP8 — Time bar

**Goal.** The Eyes-style time bar, binding WP1's API one-to-one: editable
date and clock, play/pause, the 24-detent symmetric-log rate slider, the
LIVE chip, and toasts consuming `TickReport` transitions.

**Depends on.** WP7 (widgets), WP1 (API, done).

**Read first.** ARCHITECTURE §4.2 (the exact API: `RateIndex::detents`,
`slider_pos`/`from_slider_pos`, `parse_date`/`parse_time`,
`format_date_eyes`, `TickReport`), §7, §9.5–§9.6.

**Build.**
- Date ("JUL 11, 2026"), rate label, clock as click-to-edit
  `EditableText`; strict parse via WP1 parsers; invalid input reverts the
  field and leaves the clock untouched.
- Detented slider mapped through `slider_pos`/`from_slider_pos`; drag
  emits `SimCommand::SetRate` — the same path as keyboard rate stepping.
- Play/pause; center detent = paused (RateIndex has no zero).
- LIVE chip: green dot + text when `is_live`, dimmed pill otherwise;
  click → `snap_to_live` command.
- Toasts (ui_kit toast) consuming `TickReport`: range clamp at 1800/2300,
  extrapolation notice outside 1800–2050, `snapped_live`. Transition
  events only — WP1 already guarantees this; the UI must not re-derive
  levels.

**Acceptance.**
- [x] Dragging across every detent and releasing reproduces the exact
  RateIndex ladder (round-trip through the slider mapping).
- [x] Typing an invalid date/time reverts and does not move the clock.
- [x] LIVE chip state matches `is_live` in all four regimes (paused,
  wrong rate, snapping, live).
- [x] Each toast appears exactly once per transition (scrub into the 1800
  clamp, reverse, re-enter: two clamp toasts total).

**Tests required.** Slider-detent round-trip against `RateIndex::detents`;
an edit-model test for revert-on-invalid; toast-dedup test driven by
synthetic `TickReport` sequences.

### WP9 — Labels, reticles, declutter, picking

**Goal.** Every body is findable and clickable: projected labels with the
tiered declutter ladder, contextual moon visibility, the Icons reticle
layer, and ray picking that triggers the travel tween.

**Depends on.** WP4, WP5; WP7 for label styling.

**Read first.** ARCHITECTURE §10.3 (the whole contract), §8.4 (labels are
plain UI nodes by design).

**Build.**
- Labels as Bevy UI nodes positioned per frame from `world_to_viewport`;
  wide-tracked uppercase for Sun + planets, small mixed-case + circular
  reticle for everything else.
- Declutter: priority ladder **selection › planets › dwarf planets ›
  comets › moons of the focused system › asteroids › other moons**, greedy
  screen-rect rejection; out-of-system moons label-hidden beyond a
  parent-distance threshold.
- Picking: labels are click targets; 3D picking via
  ray-vs-inflated-bounding-sphere; selection emits the travel command.

**Acceptance.**
- [x] Full-system default view: zero overlapping labels; all 8 planets
  labeled.
- [x] Focused on Jupiter: its major moons labeled; Saturn's moons not
  (until Saturn is focused/near).
- [x] Clicking a label and clicking a sphere both select and travel;
  selection always keeps its label.
- [x] Declutter is stable frame-to-frame (no label flicker while the
  camera is still).

**Tests required.** Declutter unit test on synthetic screen rects
(priority order respected; greedy rejection deterministic); picking math
test (ray vs inflated sphere hit/miss cases).

### WP10 — Left panel: Info tab, collection pages, View Options

**Goal.** The contextual left panel: per-body Info, collection navigation
("Moons of Jupiter (6)"), and View Options (size exaggeration, per-system
moon visibility, local orbit toggle).

**Depends on.** WP7, WP9 (selection); WP6 for the orbit toggle.

**Read first.** ARCHITECTURE §9.2; §4.1 (`Orbit::period_s`, categories).

**Build.**
- Info tab: name, category chip with colored dot, radius, orbital period
  (from `Orbit::period_s`; hyperbolic bodies show no period), parent link,
  curated description. Tab set is data-driven per body class.
- Collection rows navigate to collection pages; counts derived from the
  catalog at load, never hard-coded.
- View Options: exaggerate-body-sizes ×1/×10/×50 (visual only — render
  scale, never physics or picking truth), Major/All moons per system,
  local orbit-line toggle.
- Panel is collapsible; state feeds WP14 settings later (local resource
  now, persistence hook left in place).

**Acceptance.**
- [x] Iterating all 66 bodies programmatically populates the Info tab
  without panic, missing field, or empty period for elliptic bodies.
- [x] "Moons of X (n)" counts equal the catalog's actual children counts
  for every parent with moons.
- [x] Size exaggeration changes rendered radius only: picking radius and
  propagation are unaffected (test by picking at ×50).
- [x] Description shows the curated blurb; empty descriptions surface the
  WP3 lint, not a blank row.

**Tests required.** Info view-model test over the full catalog (66/66
render-ready); collection-count test against catalog topology.

### WP11 — Layers quick panel, right rail, Icons layer, UI-off mode

**Goal.** Global visibility control: the grouped layers panel, the right
rail (zoom, fullscreen, settings), and a clean UI-off presentation mode.

**Depends on.** WP7; consumes toggles from WP6 (orbits), WP9 (labels,
icons), WP4 (body categories).

**Read first.** ARCHITECTURE §9.3, §9.4.

**Build.**
- Layers panel (bottom-right, opened from the right rail) with the exact
  grouping: User Interface · Planets, Dwarf Planets, Asteroids, Comets ·
  Moons · Orbits, Labels, Icons.
- Every toggle routes through `SimCommand` and drives a central
  `LayerState` resource that WP6/WP9 rendering reads.
- UI-off: hides all HUD except a small restore affordance.
- Right rail: zoom +/− (same command path as scroll), fullscreen toggle,
  settings button (screen lands in WP14).
- Layer state persists via the WP14 settings hook (local now, wired in
  WP14).

**Acceptance.**
- [x] Every layer toggle takes effect within one frame and is reflected
  in the panel state.
- [x] UI-off leaves exactly one restore affordance; restoring returns the
  previous layout and layer states.
- [x] Zoom buttons and scroll wheel produce identical command traffic.

**Tests required.** LayerState reducer test (toggle idempotence,
group-independence); a replay-based test that a recorded toggle session
reproduces the same final LayerState hash.

### WP12 — Search + Menu browse

**Goal.** Instant, alias-aware search from the top bar, and the
full-screen Menu browse page with live counts.

**Depends on.** WP7 (top bar), WP9 (travel on select).

**Read first.** ARCHITECTURE §9.1; §4.1 (`Catalog::find` contract — the
fuzzy layer MUST preserve exact matching as a subset).

**Build.**
- Search field (`EditableText`): fuzzy, case-insensitive, alias-aware,
  instant dropdown ranked exact-prefix › alias › fuzzy; Enter travels to
  the top hit; Esc restores.
- Fuzzy layer wraps — never replaces — `Catalog::find`; an exact
  name/designation/alias match is always rank 1.
- Menu browse: full-screen page, three category columns (Planets & Moons /
  Dwarf Planets & Asteroids / Comets), curated shortlists, live counts
  derived from the catalog, expandable full lists; every entry navigates.

**Acceptance.**
- [x] "3I/ATLAS" and "C/2025 N1" both resolve uniquely to the same body;
  "hale" surfaces Hale–Bopp in the dropdown.
- [x] For every body, typing its exact name puts it at rank 1 (property
  test over all 66 × {name, designation, aliases}).
- [x] Menu counts equal catalog category counts (1/8/9/8/32/8) at load.
- [x] Keyboard-only flow works: focus search, type, Enter travels.

**Tests required.** Ranking property test over the full search-key set;
fuzzy-never-shadows-exact test; count derivation test.

### WP13 — Orbit-emphasis high-rate mode, BSC starfield, Sun bloom

**Goal.** Honest temporal aliasing handling at high rates, plus the two
scene-polish items with data dependencies: the Yale BSC starfield and the
emissive Sun with bloom.

**Depends on.** WP4, WP6 (brightness hook), WP8 (rates), WP9 (label fade).

**Read first.** ARCHITECTURE §7 (the aliasing contract, ~0.15 rad
threshold), §10.4, §10.5.

**Build.**
- Per body at catalog load, derive the phase-step-per-frame threshold
  from `Orbit::period_s`; per frame at the current rate, compute the
  parent-relative phase step; above ~0.15 rad, cross-fade the body dot and
  label out while brightening its orbit line; restore as the rate drops.
  Hysteresis so the boundary doesn't flicker.
- Onset toast ("Inner orbits shown as paths at this speed") — transition
  only, once per onset.
- Starfield: bake ~5,000 Yale BSC stars at build time (an `xtask`
  subcommand) into a point mesh on the celestial sphere with the
  equatorial→ecliptic tilt, magnitude-scaled sizes; optional faint Milky
  Way band. Record the BSC's public-domain provenance for the WP17 audit.
- Sun: emissive material + point light + bloom; low ambient for
  night-side legibility.

**Acceptance.**
- [x] At +100 yr/s the inner system reads as glowing orbit paths (no
  strobing planet dots) while Sedna and long-period comets still crawl as
  dots.
- [x] Emphasis engages/disengages per body at rates predicted from its
  period (spot-check Mercury vs Neptune) with no flicker at the boundary.
- [x] Onset toast fires exactly once per onset.
- [x] Starfield tilt is correct (ecliptic pole star-field matches
  reality: Polaris sits ~23.4° off the ecliptic pole).

**Tests required.** Threshold math unit test (rate × period → phase step);
hysteresis transition test; starfield bake test (star count, unit-sphere
positions, tilt applied).

### WP14 — Settings screen + render-recovery policies

**Goal.** Persistent settings via the 0.19 settings framework and the
render-recovery policies, with a settings UI.

**Depends on.** WP7 (widgets), WP11 (layer state), WP1's `StartMode`
(serde-ready).

**Read first.** ARCHITECTURE §8.5 (the exact persist list and recovery
policies), §4.2 (`StartMode`).

**Build.**
- `SettingsPlugin` with a reverse-domain identifier persisting: display
  mode, resolution, vsync/frame cap, quality preset, UI scale, units
  (km/mi/AU), start epoch / start-live (`StartMode`), invert axes, layer
  states.
- Settings screen (right-rail button) editing all of the above with
  ui_kit widgets; apply/revert semantics for display-mode changes.
- Render recovery: `DeviceLost → Recover`; `OutOfMemory → StopRendering`
  with a user-facing error screen. A debug command simulates device loss.
- Units setting rewires every UI distance/radius formatter (one formatter
  module; no scattered conversions).

**Acceptance.**
- [x] Every listed setting survives full quit + relaunch.
- [x] `StartMode::FixedEpoch` boots on the configured epoch;
  `StartMode::Live` boots live — both verified.
- [x] Simulated device loss recovers to a rendering app without restart.
- [x] Units toggle updates every visible distance in one frame.

**Tests required.** Settings round-trip serde test over the full struct;
formatter unit tests for all three unit modes; recovery state-machine test.

### WP15 — Texture pass, visual polish, golden screenshots

**Goal.** 2K KTX2 public-domain textures for the Sun + planets (+ major
moons as available), Saturn's ring disc, clearing the WP3 texture lints,
and the golden-screenshot harness.

**Depends on.** WP4; WP13 (bloom in goldens); real `catalog.ron`
(texture fields flow through the manifest → regeneration, never
hand-edits of the RON).

**Read first.** ARCHITECTURE §10.1, §12 (goldens), §1 (legal boundary —
attribution, no NASA branding).

**Build.**
- Source 2K NASA SVS / USGS public-domain textures; convert to KTX2 via
  an `xtask` subcommand; per-asset license/source metadata file, checked
  by a CI script (the WP17 audit input).
- Texture assignment lives in the curated manifest; regenerate the
  catalog to populate `texture` fields (invariant 3 — no hand-editing the
  RON).
- Saturn ring: translucent disc with a ring texture.
- Golden screenshots: six canonical views (defined in a doc alongside the
  harness), captured per render backend in CI, compared with a perceptual
  threshold.

**Acceptance.**
- [x] `catalog.lint()` reports zero untextured star/planet lints.
- [x] Every shipped texture has license + source metadata; the CI check
  fails on a metadata-less asset (prove by adding one in a scratch branch).
- [x] Untextured bodies still render with catalog colors (texturing stays
  polish, not a dependency).
- [x] Goldens are stable across two consecutive CI runs on the same
  platform.

**Tests required.** The metadata CI check itself; golden harness with the
six views; KTX2 pipeline smoke test (round-trip one texture).

### WP16 — Steam release engineering

**Goal.** Feature-gated Steamworks integration, the overlay spike, and the
full packaging/signing/depot pipeline — dry-run end-to-end, not at ship.

**Depends on.** WP14 (settings/recovery), WP15 (assets to package);
Steamworks App ID from the human.

**Read first.** ARCHITECTURE §11 (the whole section is the brief).

**Build.**
- `SteamPlugin` behind cargo feature `steam`, wrapping `steamworks`: init
  with App ID, shutdown on exit, nothing else — all calls behind a small
  `PlatformServices` trait so default builds compile without Steam.
- **Overlay spike first** (top risk, esp. Metal): document works/doesn't
  per OS; the app MUST NOT require the overlay either way.
- Packaging in `xtask`: macOS universal (`lipo`), `.app` bundle, Developer
  ID signing + notarization + stapling — full dry-run; Windows signed exe
  + assets; SteamPipe depots (`macos`, `windows-x64`) with
  `dev → beta → default` branches.

**Acceptance.**
- [ ] Default (non-`steam`) build has no Steamworks in its dependency
  tree (CI-checked like core purity).
- [ ] Overlay spike results documented in `docs/` for both OSes; app runs
  correctly with overlay unavailable.
- [ ] Sign/notarize/staple dry-run passes on macOS; a `dev`-branch
  SteamPipe install launches on both OSes.
- [ ] Bundle ≤ 150 MB/platform measured and recorded.

**Tests required.** `PlatformServices` mock test (app logic never calls
steamworks directly); packaging script smoke run in CI (unsigned variant).

### WP17 — QA: replay suite, perf gates, demo script, licensing audit

**Goal.** The release gates of ARCHITECTURE §13, exercised and recorded.

**Depends on.** Everything through WP16.

**Read first.** ARCHITECTURE §12, §13, §14.

**Build.**
- Replay suite: a library of recorded sessions (incl. the demo script)
  replayed in CI on both OSes with state-hash assertions.
- Perf gates: 60 fps with all layers on, measured on an M1 MacBook Air
  and a GTX 1650-class laptop; capture traces and record numbers.
- Demo script automation: 2026 start → search "Sedna" → travel to
  full-system view → Menu browse to Jupiter → moons + View Options →
  scrub to Halley's 1986 perihelion at −3 yr/s → +100 yr/s
  orbit-emphasis → LIVE snap.
- Licensing audit: fonts, textures, star data, no NASA branding —
  checklist doc, signed by the human.

**Acceptance.**
- [ ] Demo script passes end-to-end unattended on both OSes.
- [ ] Perf numbers recorded for both reference machines; both ≥ 60 fps
  all-layers.
- [ ] Replay suite green in CI on both OSes.
- [ ] Licensing audit checklist complete with human sign-off recorded in
  the change log.

**Tests required.** The suite *is* the tests; additionally a CI job that
fails if any replay session is skipped.

**Reference-machine window gate.** Before WP17 closeout, the M1 MacBook Air
must pass `cargo run -p solar-sim --release -- --smoke 60 --expect-backend
metal --assert-nonblack`, and the GTX 1650-class Windows laptop must pass the
same command with `dx12`. This is an opt-in real-hardware gate, not a hosted-CI
gate; it does not check any WP17 acceptance box by itself.

### WP18 — Compare Size mode (deferred)

Optional post-beta. No brief until un-deferred by the human.

---

## Next up (dependency order)

1. **WP5 → WP6**, then **WP7/WP8** (ui_kit, then the time bar
   binding WP1's API), then WP9–WP15 per briefs, WP16–17 release
   engineering.

## Open questions (humans close these)

| # | Question | Raised | Status |
|---|---|---|---|
| Q1 | Confirm Bevy 0.19.x minimum Rust toolchain and record in WP0 pin | 2026-07-12 | **closed 2026-07-12** — crates.io reports `rust_version = 1.95.0` for bevy 0.19.0 (and all 0.19 RCs); `rust-toolchain.toml` pins `channel = "1.95.0"`, while `crates/sim-core/Cargo.toml` retains `rust-version = "1.75"`. Evidence: `docs/open-questions-brief-2026-07-12.md` §Q1; commit `61896e8`. |
| Q2 | TNO GM values (Pluto 869.6 / Eris 1108 / Haumea 267 km³/s²) — accept or replace with cited values during curated review? Includes the Pluto-GM semantics decision (Pluto-only vs Pluto+Charon ≈ 975.5 for correct Charon period under μ=parent-GM). | 2026-07-12 | **closed 2026-07-13** — human approved Pluto+Charon system GM 975.5 km³/s² (869.6 + 105.9); Eris/Haumea retain their system values; provenance must state the choice. |
| Q3 | 3I/ATLAS nucleus radius: literature is uncertain; which value + citation ships? | 2026-07-12 | **closed 2026-07-13** — human approved adopted R = 0.5 km with the HST 0.16–2.8 km range and NGA-based estimate cited in provenance. |
| Q4 | Constellation-figure line set licensing (fast-follow; Yale BSC-derived in-house vs licensed) | 2026-07-12 | open — options + recommendation in brief §Q4 (recommend in-house over public-domain BSC) |
| Q5 | **Horizons planet routes: switch giant planets from planet centers (599/699/799/899) to system barycenters (5/6/7/8)?** The 2026-07-12 online run failed at Jupiter (`no $$SOE`). Planet-center ephemerides are defined by satellite solutions with limited time spans, while barycenters cover ±9999 yr, and JPL's own manual recommends barycenters for osculating-element output. Giant-planet vs own-barycenter offset ≤ ~100 km — far under two-body display budgets. Requires: manifest route edit, ARCHITECTURE §5.3 wording (human edit), dry-run/spec text updates. Raw capture/diagnostics are now implemented; the JD 2561120 probe confirmed Jupiter-center ends in 2200 while barycenter 5 returns ELEMENTS. Full analysis in brief §Q5. | 2026-07-12 | **closed 2026-07-13** — human approved and saved ARCHITECTURE §5.3; Mercury–Mars remain geometric centers and Jupiter–Neptune now use system barycenters. The mean-motion/secular-fit and SBDB normalization contracts remain binding. |
| Q6 | **Spot-check epoch semantics after real-vector calibration:** require all 10 bodies at both 2026 and 1986, or use the 2026 point for the full set plus the 1986 point only for Halley? The 20-point interpretation forces non-physical pass budgets (Earth is 144,450,813.9 km off in 1986 because the approved near-pair unwrapped-MA slope is 1.335394656°/day; Phoebe is 14,762,084.4 km off under the declared no-secular moon model). | 2026-07-13 | **closed 2026-07-13** — human approved the full 10-body gate at the catalog epoch plus Halley at its 1986 demo/perihelion epoch, retained all 20 vectors as audit data, and explicitly approved the 1 km planet / 10 km moon / 25,000 km dwarf / 30,000,000 km comet budgets. |
| Q7 | **Approve the general Sun/planet GM audit?** Adopt the exact JPL DE440 set in `docs/wp3-gm-audit-2026-07-13.md`: eight numeric replacements, with Venus verified unchanged; Mars–Neptune use DE440 system GMs. | 2026-07-13 | **closed 2026-07-13** — human approved all nine rows. The eight replacements and verified Venus value are applied with DE440 provenance; both catalogs were regenerated from captured responses and the active position gate remains green. |
| Q8 | **What defines “Major” in WP10's per-system Major/All moon visibility option?** The frozen catalog has no major-moon flag and ARCHITECTURE gives no membership list or physical cutoff. Recommend an additive curated boolean in the generator manifest/schema so the choice is reviewable; alternatives are a human-approved id set or a specified radius rule. | 2026-07-13 | **closed 2026-07-13** — human approved the recommended additive, catalog-backed curated boolean. The manifest's explicit 24-id membership list is the source of truth and covers every modeled moon system. |
| Q9 | **Approve NASA HEASARC BSC5P as WP13's license-clean Bright Star Catalog source?** The authoritative NASA Open Data Portal identifies `ivo://nasa.heasarc/bsc5p`, marks access public, and links its license to the U.S. government-works policy. The table is HEASARC's 1995 derivative of ADC/CDS V/50 with later position corrections. Recommend the NASA export rather than redistributing CDS V/50 directly; retain Hoffleit & Warren, HEASARC, NASA Open Data, and V/50 provenance in the audit sidecar. | 2026-07-14 | **closed 2026-07-14** — human approved NASA HEASARC BSC5P. The derived 5,000-star asset and `assets/starfield-SOURCE.md` record the exact TAP query, hashes, government-works license route, catalog references, exclusions, and bake transform. |
Q10 — CLOSED (human, 2026-07-14). WP15's "stable across two consecutive CI runs
on the same platform" is scoped to a SINGLE platform, and that platform is
macOS/Metal. Rationale: hosted windows-latest has no GPU, so wgpu falls back to
the WARP software rasterizer. Proving WARP is deterministic across two runs is
near-tautological and tests nothing the Metal run does not. Real-GPU DX12 golden
validation is deferred to WP16/WP17 bring-up on real hardware. DX12 captures stay
in the golden workflow as a non-blocking code-path check. No acceptance text is
reworded; this records how the existing text is read.

Q11 — CLOSED (human, 2026-07-14). Yes, create the platform matrix. Docker is not
required: the repository is public, so GitHub-hosted standard runners are free
and unlimited. Validate the Linux lane on hosted CI by pushing a branch and
reading the run. Self-hosted runners and local VMs are OFF the table.

Q12 — OPEN. The 2026-07-14 CI follow-up instructs agents to work tasks CI-1
through CI-6 in order, but does not define the scope, acceptance evidence, or
commands for any of those six tasks. Provide the exact CI-1 through CI-6 briefs;
agents must not infer them from the superseded private-repository Task 1/Task 2
numbering.

## Change log (append-only; newest first)

- **2026-07-14** — CI-5 closed WP15 on clean, pushed `main` commit
  `887a2c60bf2cc04f0817bcd215eda9fa9075601b`. Two separate
  `workflow_dispatch` executions used `backend=metal`, job label
  `goldens (metal)`, and runner label `macos-14` at that exact SHA:
  [goldens #1 / run 29335082428](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29335082428)
  passed in 2m10s, then
  [goldens #2 / run 29335434467](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29335434467)
  passed in 2m48s with no intervening code or workflow configuration change.
  Per-view results below are `(mean Delta E, p99 Delta E, run-a/run-b
  attempts)`. Run 29335082428: `full-system` (0.0048, 0.0000, 1/1),
  `inner-orbits` (0.0000, 0.0000, 1/1), `earth-texture` (0.0000, 0.0000,
  1/1), `jupiter-system` (0.0050, 0.0000, 1/1), `saturn-rings` (0.0000,
  0.0000, 1/1), and `sun-bloom` (0.0000, 0.0000, 1/1). Run 29335434467:
  `full-system` (0.0053, 0.0000, 1/1), `inner-orbits` (0.0000, 0.0000,
  1/1), `earth-texture` (0.0000, 0.0000, 1/1), `jupiter-system` (0.0000,
  0.0000, 1/1), `saturn-rings` (0.0000, 0.0000, 1/1), and `sun-bloom`
  (0.0000, 0.0000, 1/1). Neither execution used `--allow-retries`.
  `git status --short --branch` and `git rev-parse HEAD main origin/main`
  confirmed clean synchronized `main`; pre-merge hosted CI
  [#33](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29334369406)
  passed all five jobs, and the post-golden local `cargo test` passed all 200
  tests. This evidence satisfies the unchanged final WP15 acceptance criterion;
  WP15 is now **✅ done**.
- **2026-07-14** — CI-4 complete: successful golden captures now print
  `golden-attempts view=<slug> attempts=<n>`; `xtask capture-goldens` validates
  one record per canonical view, writes the six counts to
  `golden-attempts.txt` beside the PPMs, prints a summary, and includes the
  manifest in the uploaded run-b artifact. `xtask compare-goldens` reports
  baseline/candidate counts per view and rejects any count above one by default;
  `--allow-retries` is documented and implemented as an explicit diagnostic-only
  escape hatch. Two local `capture-goldens` Metal runs each printed attempts 1
  for all six views, and the default comparison printed attempts `1/1` with all
  views passing. After changing the ignored candidate manifest's earth count to
  2, the default comparison printed attempts `1/2` and exited 1; the same command
  with `--allow-retries` exited 0. Hosted run
  [#31](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29332811125)
  passed in 7m48s: `lint` 48s, `test-linux` 1m58s, `invariants` 32s,
  `platform (macos-14)` 2m37s, and `platform (windows-latest)` 7m44s. Local
  evidence: `cargo test` passes all 200 tests; `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `scripts/check-texture-metadata.sh`, both fixture catalog commands, workflow
  YAML parsing and boundary scans, and `git diff --check` pass. No dependency,
  read-only file, generated catalog, curated route, catalog composition,
  capture-attempt/settle constant, Delta E threshold, or WP15 acceptance
  checkbox changed.
- **2026-07-14** — CI-3 complete: added dependency-free
  `--reject-software-adapter` handling that reports the adapter name and
  `device_type` in smoke output and exits nonzero for a reported `Cpu` adapter.
  Metal golden children launched by `xtask capture-goldens` now receive the
  same guard; DX12 golden children and the hosted Windows smoke do not. The
  future real-DX12 acceptance command is recorded in the WP15 guide. Because
  CI-2 macOS runs [#27](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29328432241)
  and [#28](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29329026027)
  were green, the macOS smoke is now a hard gate while Windows remains
  `continue-on-error: true`. Hosted run
  [#29](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29330775705)
  passed in 6m00s: `lint` 41s, `test-linux` 1m58s, `invariants` 30s,
  `platform (macos-14)` 2m32s, and `platform (windows-latest)` 5m56s. macOS
  printed `Apple Paravirtual device`, `device_type IntegratedGpu`, backend
  `metal`, 1.798s/26.7 fps, and a passed expectation; Windows printed
  `Microsoft Basic Render Driver`, `device_type Cpu`, backend `dx12`,
  12.808s/3.7 fps, and a passed expectation. Local evidence:
  `cargo run -p solar-sim --release -- --smoke 60 --expect-backend metal
  --reject-software-adapter` exited 0 on `Apple M2 Pro`/`IntegratedGpu`/Metal
  at 162.4 fps; `cargo test` passes all 198 tests; `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, YAML parsing,
  texture metadata, fixtures catalog smoke, workflow boundary scans, and `git
  diff --check` pass. No dependency, read-only file, generated catalog,
  curated route, catalog composition, or WP15 acceptance checkbox changed.
- **2026-07-14** — CI-2 complete: landed the backend-checked smoke CLI and
  wired 60-frame window launches after the `platform` release build, with both
  macOS/Metal and Windows/DX12 steps kept `continue-on-error: true`. Hosted
  run [#27](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29328432241)
  passed every job. macOS printed `smoke: completed 60 update frames; measured
  48 after 12 warmup frames in 1.727s (27.8 fps)`, identified `Apple
  Paravirtual device` on `metal`, and passed the Metal expectation. Windows
  printed the same completion line with 38.986s (1.2 fps), identified
  `Microsoft Basic Render Driver` on `dx12`, and passed the DX12 expectation;
  Bevy also warned that the selected adapter is software-only and very slow,
  matching the retained WARP comment. The smoke steps succeeded in 13s and
  2m06s respectively, but remain non-blocking pending CI-3. Local evidence:
  pre- and post-change `cargo test` pass all 196 tests; `cargo fmt --all --
  --check`, `cargo clippy --workspace --all-targets -- -D warnings`, YAML
  parsing, `git diff --check`, and the online-feature/self-hosted/
  `pull_request_target` workflow scans pass. No dependency, read-only file,
  generated catalog, curated route, catalog composition, or WP15 acceptance
  checkbox changed.
- **2026-07-14** — CI-1 complete: split the public-repository gates into
  `.github/workflows/ci.yml` (push to `main` plus pull requests) and the
  dispatch-only `.github/workflows/goldens.yml`; added the full hosted
  Linux/macOS/Windows lanes, scoped cancellation to `ci.yml`, restricted both
  workflows to `contents: read`, pinned every third-party action to a commit
  SHA, and retained the unchanged purity/offline/online-feature guards. Hosted
  run [#24](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29323761630)
  found the Linux Bevy build also requires `libwayland-dev` (`wayland-client.pc`
  was absent); after adding that package to `lint` and `test-linux`, hosted run
  [#25](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29323939171)
  passed: `lint` 4m58s, `test-linux` 19m48s, `invariants` 27s,
  `platform (macos-14)` 30m25s, and `platform (windows-latest)` 58m37s. No
  headless test failed. Local evidence: `cargo test` and `cargo nextest run
  --workspace` each pass all 196 tests; `cargo fmt --all -- --check`, `cargo
  clippy --workspace --all-targets -- -D warnings`,
  `scripts/check-texture-metadata.sh`, the fixtures `gen-catalog` smoke, `cargo
  build -p solar-sim --release`, the standalone Cargo 1.75 `sim-core` check,
  and the unchanged workflow purity/offline/online-feature scans pass. No
  dependency, read-only file, generated catalog, curated route, catalog
  composition, or WP15 acceptance checkbox changed.
- **2026-07-14** — Added honest shipped-window smoke coverage locally while
  leaving CI placement blocked on Q11. `--smoke N` now accepts only the reviewed
  `--expect-backend metal|dx12|vulkan` values, compares them with Bevy's
  `RenderAdapterInfo`, and propagates every non-success `AppExit` as a nonzero
  process exit. The opt-in `--assert-nonblack` path waits until frame N, reads
  the primary window render target, and exits nonzero on an all-black RGB image;
  it remains excluded from hosted gates and is recorded as a required WP17
  reference-machine check. On the local Apple M2 Pro, the 60-frame Metal gate
  passed at 202.4 fps; the opt-in run passed at 210.7 fps with a nonblack
  1920×1200 primary-window readback. A deliberate DX12 expectation on the same
  machine reported `expected backend dx12, got metal` and exited 1. Evidence:
  `cargo test` passes all 196 tests (53 `sim-core`, 100 `solar-sim`, 40 `xtask`
  lib, two smoke, one active spot-check); `cargo fmt --all -- --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` pass. No dependencies,
  read-only files, generated catalogs, curated routes, or catalog composition
  changed. The requested CI commands are not yet placed because Task 1 stopped
  before creating the referenced `platform` matrix when Docker was unavailable;
  Q11 records the required human decision.
- **2026-07-14** — Human ruling on the WP15 acceptance text. "Stable across two
  consecutive CI runs **on the same platform**" is scoped to a single platform,
  and that platform is macOS/Metal. Rationale: the DX12 leg runs on
  `windows-latest`, which has no GPU — wgpu falls back to the WARP software
  rasterizer (see the standing comment in the `build-windows` job). Proving WARP
  is deterministic across two runs is near-tautological and tests nothing the
  Metal run does not. Real-GPU DX12 golden validation is deferred to WP16/WP17
  bring-up on the GTX 1650-class reference laptop. DX12 captures remain in the
  golden workflow as a non-blocking code-path check. No acceptance text is
  reworded; this records how the existing text is to be read.
- **2026-07-14** — Repaired the WP15 CI run #21 golden-capture failure without
  weakening its gate. `cargo run -p xtask` exported xtask's
  `CARGO_MANIFEST_DIR` into each `solar-sim` child, so Bevy resolved
  `../../assets` from `xtask/` and rejected every texture plus the Inter font.
  The launcher now pins the child environment to the `solar-sim` manifest;
  a regression test independently resolves that inherited path to the
  workspace asset root. Golden cameras now render to a fixed 960×600 sRGB
  image target instead of reading a window swapchain, eliminating the
  repeatable all-black Metal readback while still exercising the selected
  Metal/DX12 backend. HUD-free views hide their surfaces rather than queuing
  overlapping hierarchy despawns. Two complete local Metal runs each captured
  all six canonical views, and the CI-equivalent comparator reports maximum
  mean Delta E = 0.0050 and p99 Delta E = 0.0000 across the final set, far
  inside the 1.25/4.0 gate. Evidence: `cargo test` passes all 194 tests (53
  `sim-core`, 98 `solar-sim`, 40 `xtask` lib, two smoke, one active
  spot-check); `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, and the release build pass. WP15 and its final
  acceptance box remain **in-progress** until the repaired commit produces two
  consecutive passing hosted Metal/DX12 runs on the same platform.
- **2026-07-14** — WP15 implementation complete; hosted golden stability
  verification remains. Added 15 catalog-driven 2048×1024 sphere KTX2 assets
  plus Saturn's 2048-pixel translucent ring strip, with byte-exact public-domain
  NASA/USGS metadata for all 16 assets. Texture identity now flows through the
  curated manifest and both catalogs were regenerated through `xtask`; the real
  catalog has zero untextured star/planet lints. The renderer preserves catalog
  colors when an assignment/asset server is absent, preserves source texels when
  present, and parents a seam-closed, double-sided annulus to Saturn. The offline
  pipeline now converts strict PPM to KTX2, round-trips the result, hashes source
  and output bytes, and rejects orphaned or invalid metadata; the positive audit
  passes all 16 assets; inserting a temporary `metadata-less.ktx2` beside them
  makes the exact CI script exit 1 with the expected missing-sidecar error, and
  the audit passes again after removing that scratch asset. The
  golden harness defines exactly six fixed 960×600 views, waits for texture and
  pipeline settling, rejects all-black readback, compares CIE Lab Delta E per
  backend, and runs twice for Metal/DX12 in CI. Evidence: `cargo test` passes all
  191 tests (53 `sim-core`, 96 `solar-sim`, 39 `xtask` lib, two smoke, one active
  spot-check); `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `scripts/check-texture-metadata.sh`, `cargo run
  -p xtask -- gen-catalog --dry-run`, and `cargo build -p solar-sim --release`
  pass. No dependencies were added. WP15 stays **in-progress** and the final
  acceptance box remains unchecked until two consecutive hosted CI runs on the
  same platform prove the Metal/DX12 captures stable; rapid repeated local Metal
  launches also exercised the black-frame guard and correctly failed rather
  than accepting an invalid golden.
- **2026-07-14** — Started WP15 from the green 178-test WP14 checkout after
  reading ARCHITECTURE §§1, 10.1, and 12. The implementation will keep every
  source assignment in the curated generator manifest, normalize public-domain
  NASA SVS / NASA 3D Resources / USGS imagery through a dependency-free KTX2
  `xtask` pipeline, verify per-asset provenance plus output digests in CI, add
  Saturn's textured translucent ring mesh, and make the six canonical golden
  views deterministic at a fixed epoch with per-backend perceptual comparison.
  Evidence: pre-change `cargo test` passes all 178 tests; the checkout was clean;
  no read-only files or generated catalogs were edited.
- **2026-07-14** — WP14 done. Enabled Bevy 0.19's native settings feature
  under `com.github.jiayanzeng.solar-sim`; its reflected `AppSettings` schema
  persists display mode, resolution, vsync/frame cap, quality, UI scale,
  km/mi/AU units, fixed/live `StartMode`, both orbit-axis inversions, and all
  nine layer switches. Loading happens before clock/catalog initialization;
  deferred saves plus a synchronous window-close save cover normal quit paths.
  The settings screen exposes all 38 controls through `ui_kit` scenes with
  AccessKit labels and stages display changes until Apply; Revert is tested.
  A child-process test writes the full non-default schema under an isolated
  HOME and reloads it in a fresh process. Both start modes, layer restoration,
  full serde round-trip, and a one-update km→mi radius repaint are covered.
  `formatting.rs` is now the sole visible-distance conversion boundary and
  uses the catalog's `AU_KM`. The native renderer policy returns `Recover` for
  `DeviceLost` and `StopRendering` for OOM, with an error overlay and window
  title fallback; F9 and `--simulate-device-loss` use the recorded
  `SimCommand` path. A real destroyed-device probe recovered and completed all
  180 smoke frames at 120.0 fps without restart; the ordinary 60-frame smoke
  completed at 115.3 fps. Evidence: `cargo test` passes all 178 tests;
  `cargo fmt --all -- --check` is clean; `cargo clippy --workspace
  --all-targets` has zero warnings. The only added direct test dependencies
  are `serde` and `ron`, used solely by the required settings round-trip test;
  the production dependency change only enables the architecture-mandated
  `bevy_settings` feature on the existing Bevy dependency. No read-only or
  generated catalog files changed.
- **2026-07-14** — Started WP14 from the green 167-test WP13 checkout after
  reading ARCHITECTURE §§4.2 and 8.5. The implementation will use Bevy 0.19's
  feature-gated `SettingsPlugin` with a reverse-domain identifier, load the
  persisted resource before clock/catalog initialization, centralize every
  user-visible length conversion, route the existing right-rail request into
  a code-defined `ui_kit` settings surface, and model device-loss/OOM recovery
  independently of backend event delivery so both paths are deterministic and
  testable. Enabling the mandated `bevy_settings` feature on the already
  approved Bevy dependency adds no direct dependency. Evidence: pre-change
  `cargo test` passes all 167 tests; no read-only files changed.
- **2026-07-14** — WP13 done after human approval of Q9's NASA HEASARC
  BSC5P route. Queried only `hr,ra,dec,vmag`, in HR order, from the official
  TAP endpoint; the source response is SHA-256
  `e4f539290c7f6303695f6fafb07618d66a23ce55e5898cb1145769aaa0913b6f`.
  The schema-pinned VOTable BINARY parser validates the query status, exact
  four-field types/units, base64 stream, ranges, HR ordering, and the full
  9,110-row/9,096-star composition. It explicitly excludes HEASARC's 14
  historical non-stellar HR rows with null magnitudes. The deterministic bake
  commits the brightest 5,000 stars as `assets/starfield.bsc` (SHA-256
  `312d6b4a94f0fd62e4877f7c63d36ba8af7ac084537f05d07faead3ef6fd628b`)
  with full source/license/transform metadata in `assets/starfield-SOURCE.md`;
  a replay bake compares byte-identically. The app renders it as one retained,
  camera-centered 20,000-vertex/30,000-index point-sprite mesh, so magnitude
  sizes remain visible without per-star entities and the floating origin
  cannot induce parallax jitter. The real-asset test verifies 5,000 finite
  unit-sphere points and Polaris at ~23.4° from the ecliptic pole. At +100
  yr/s/60 fps, catalog-period tests prove Mercury through Saturn fade fully to
  bright paths while Uranus, Neptune, Sedna, Halley, and Hale–Bopp remain dots;
  3I/ATLAS has no elliptic threshold. Hysteresis and onset tests prove no
  boundary flicker and exactly one toast per onset. Sun emissive/PBR, the sole
  point light, bloom, and low ambient are entity-tested. Evidence: `cargo test`
  passes all 167 tests (53 + 80 + 31 + 2 + 1); `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `git diff --check`,
  the catalog dry-run, and deterministic starfield rebake pass. The 120-frame
  GPU smoke rendered all bodies, orbits, and the retained starfield at 120.2
  fps after warmup. No dependency or read-only-file changes.
- **2026-07-14** — WP13 implementation is complete except for the Q9-gated
  production star payload. Added catalog-load elliptic period thresholds,
  per-frame parent-relative phase-step checks at the WP8 rate, 0.15/0.12 rad
  hysteresis, wall-time dot/label cross-fades, per-body WP6 orbit brightening,
  and a transition-only onset message consumed by the existing time-bar toast.
  At +100 yr/s/60 fps the independent period cross-check engages Mercury but
  leaves Neptune, Sedna, Halley, Hale–Bopp, and hyperbolic 3I/ATLAS as dots.
  The Sun now has the sole point light and stronger emissive material; planets
  use lit PBR materials, the camera carries Bevy bloom, and a low cool ambient
  fill preserves night sides. Added the offline `xtask bake-starfield` fixed-
  width parser, deterministic brightest-5,000 ecliptic-J2000 bake, validated
  binary format, and one retained magnitude-scaled point-sprite mesh centered
  on the camera for floating-origin stability. Synthetic tests cover corrupt
  rows, exactly 5,000 finite unit-sphere records, binary round-trip, Polaris'
  ~23.4° ecliptic-pole separation, and magnitude sizing. The matching runtime
  loader is ready for `assets/starfield.bsc`; it remains deliberately absent
  pending Q9. Evidence: `cargo test` passes all 165 tests (53 + 78 + 31 + 2 +
  1); `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --
  -D warnings`, and `git diff --check` pass. No dependencies or read-only files
  changed. NASA Open Data records BSC5P as public and licenses it via the U.S.
  government-works policy.
- **2026-07-14** — Started WP13 from the green 156-test WP12 checkout after
  reading ARCHITECTURE §§7 and 10.4–10.5. Orbit emphasis will derive elliptic
  period thresholds at catalog load, use per-body hysteresis and wall-time
  cross-fades, and emit one global onset transition into the existing toast
  surface; dots, labels, and WP6 orbit brightness remain render-only. Sun
  lighting will use Bevy's existing emissive/PBR/bloom stack with no new
  dependency. Raised Q9 before importing BSC data because the authoritative
  HEASARC/CDS distributions do not substantiate ARCHITECTURE's public-domain
  label and NASA explicitly requires validation of third-party archive rights.
  Evidence: pre-change `cargo test` passes all 156 tests; HEASARC BSC5P
  provenance and NASA Science Data Portal licensing guidance.
- **2026-07-14** — WP12 done. Added a deterministic catalog search layer that
  seeds exact matches through `Catalog::find`, then ranks name/designation
  prefixes, aliases, subsequences, and bounded edit-distance candidates while
  retaining one best hit per body. The top-bar `EditableText` now provides an
  accessible eight-result instant dropdown, Enter and click travel through
  `SimCommand`, and Esc restores the pre-edit value. Added an accessible
  full-screen Menu with the exact three category columns, explicit curated
  shortlists, catalog-derived 1/8/9/8/32/8 live counts, scrolling complete
  lists, and travel actions for all 66 entries. Seven regressions cover every
  exact name/designation/alias key, 3I/ATLAS identity preservation, Hale–Bopp
  prefix and typo fuzzing, deterministic ranking, count/list derivation,
  curated-to-complete menu rendering, edit restoration, and a real focused
  keyboard Enter event, raising the workspace from 149 to 156 tests. Evidence:
  `cargo test`, `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `xtask gen-catalog --dry-run`, and `git diff --check` pass; the final native
  120-frame full-system smoke measured 118.8 fps after warmup. No dependency,
  generated catalog, read-only file, or catalog composition changed.
- **2026-07-14** — Started WP12 from the clean, green 149-test WP11
  checkout after reading ARCHITECTURE §§4.1 and 9.1. Search will wrap the
  exact `Catalog::find` contract, then deterministically rank prefix, alias,
  and fuzzy candidates without changing catalog data. Search and browse
  activation will enqueue the existing travel command; menu counts and
  expandable lists will be derived from the loaded 66-body catalog. Evidence:
  pre-change `cargo test` passes all 149 tests.
- **2026-07-14** — WP11 done. Added a central nine-layer `LayerState` reducer
  with stable replay ids/hash and an explicit WP14 persistence snapshot; every
  layer, fullscreen, settings, and rail-zoom action crosses `SimCommand`.
  Built the accessible right rail and the exact four-group bottom-right panel
  (User Interface · four body categories · Moons · Orbits/Labels/Icons), plus
  borderless-fullscreen and WP14 settings-request hooks. Category switches now
  drive body visibility, WP6 consumes Orbits, and WP9 independently lays out
  Labels and circular-reticle Icons. UI-off suppresses every tagged HUD/label
  surface after rebuilds and exposes exactly one restore control; restoration
  preserves the open-panel layout and all unrelated layer values. Eleven
  regressions cover idempotence/group independence, all nine one-update
  transitions, exact grouping/accessibility, body/orbit/label/icon consumers,
  restore behavior, rail command traffic, fullscreen, persistence, malformed
  replay rows, and recorded-session layer-hash equality, raising the workspace
  from 138 to 149 tests. Evidence: `cargo test`,
  `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `xtask gen-catalog --dry-run`, and `git diff --check` pass; the final native
  180-frame Jupiter smoke measured 118.3 fps after warmup. No dependency,
  generated catalog, or read-only file changed.
- **2026-07-14** — Started WP11 from the clean, green 138-test WP10
  checkout after reading ARCHITECTURE §§9.3–9.4. Layer mutations will cross
  the existing `SimCommand` queue and reduce into one persistence-ready
  resource; WP6/WP9 visibility and every HUD surface will consume that state.
  The rail's zoom buttons will enqueue the same `Dolly` command as wheel input,
  while fullscreen and the WP14 settings placeholder remain render/UI concerns.
- **2026-07-13** — WP10 done after human approval closed Q8. Added the
  backward-compatible `BodyRecord::is_major_moon` schema field, rejects its use
  on non-moons, and derives every emitted value from a centralized 24-id
  manifest list covering all nine modeled moon systems. Both committed catalogs
  were regenerated through offline `xtask` fixtures with the display
  classification in each moon's provenance. The enabled Major/All control now
  filters spheres, labels, picking eligibility, and orbit lines per system; a
  selected minor moon remains visible, and All restores every modeled moon.
  Five new regressions cover schema/default validation, exact unique manifest
  membership and system coverage, rendered visibility, label filtering,
  orbit filtering, and restoration, raising the workspace from 133 to 138
  tests. Evidence: `cargo test`, `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `xtask gen-catalog --dry-run`, and `git diff --check` pass; the final
  120-frame Jupiter native smoke measured 118.9 fps after warmup. No dependency
  or read-only fixture changed.
- **2026-07-13** — Implemented WP10's unambiguous surface while leaving the
  package in progress on Q8. Added the collapsible contextual panel, data-driven
  Info/Collection/View Options tabs, exact catalog lint surfacing for empty
  descriptions, elliptic/hyperbolic period models, parent travel links, and
  catalog-derived moon pages. View settings now have a WP14 snapshot/restore
  seam; ×1/×10/×50 scales only `BodyVisual`, and per-body local-orbit visibility
  feeds the existing orbit renderer. All is the honest default moon mode and
  Major is visibly disabled until Q8 supplies authoritative membership. Six
  regressions cover all 66 Info models, topology-derived collection counts,
  ×50 render/propagation/picking separation, settings round-trip, collection
  navigation, and local-orbit isolation, raising the workspace from 127 to 133
  tests. Exact-app visual probes exercised Jupiter's Info, six-moon collection,
  and View Options surfaces. Evidence: `cargo test`,
  `cargo fmt --all -- --check`, and
  `cargo clippy --workspace --all-targets -- -D warnings` pass; the final
  120-frame Jupiter native smoke measured 120.0 fps after warmup. No dependency
  or generated catalog asset changed.
- **2026-07-13** — Started WP10 from the green 127-test WP9 checkout after
  reading ARCHITECTURE §§4.1 and 9.2. The Info and collection models will be
  catalog-derived, empty descriptions will display the existing WP3 lint, and
  visual size/orbit options will remain outside propagation and picking truth.
  Raised Q8 because neither the schema nor architecture defines Major-moon
  membership; no cutoff or id list will be improvised.
- **2026-07-13** — WP9 done. Added 66 accessible projected label buttons,
  wide-tracked uppercase Sun/planet labels, mixed-case circular-reticle labels
  for the other 57 bodies, deterministic priority/catalog-index declutter,
  stable alternative slots for the selected body, planets, and focused-system
  moons, and an outermost-moon-apoapsis context gate. The full-system default
  framing is now derived from the outermost planet, while planet travel reuses
  the established four-radius framing rule over the complete modeled moon
  system; an exact-app visual probe showed all eight planets without overlap
  and Jupiter plus Io, Europa, Ganymede, Callisto, Amalthea, and Himalia with no
  Saturn moon labels. Label activation and a transparent viewport pick surface
  both enqueue the existing travel command; the latter resolves the nearest
  analytic ray/sphere hit with a 10-pixel minimum pick radius. Eight regressions
  cover priority/determinism, clustered planet and focused-moon layouts,
  contextual gating, hit/miss/tangent/inflation math, shared label travel,
  AccessKit/reticle counts, and data-derived framing, raising the workspace
  from 119 to 127 tests. The portable replay hash changed only because the
  intentional Jupiter travel target now frames its moon system; record/replay
  equality remains asserted, so its golden was updated to
  `11614332433107791956`. Evidence: `cargo test`, `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `git diff --check`,
  and `xtask gen-catalog --dry-run` pass. Final native smokes rendered the
  180-frame full-system scene at 119.8 fps and the 120-frame Jupiter scene at
  118.9 fps after warmup; no dependency or generated catalog asset changed.
- **2026-07-13** — Started WP9 from the green 119-test WP8 checkout after
  reading ARCHITECTURE §§8.4 and 10.3 plus the invariant/frame-flow contracts.
  Labels will remain plain projected Bevy UI nodes; deterministic priority and
  contextual moon gating will be render-side only, while label and sphere
  picks enqueue the existing travel command without mutating selection state.
- **2026-07-13** — WP8 done. Added `TimeBarPlugin` with Eyes-format date and
  clock `EditableText` fields, strict WP1 parser commits with bit-identical
  invalid-edit reversion, play/pause, the 24 signed `RateIndex` detents plus
  the separate paused center, and a semantic LIVE pill driven directly by
  `SimClock::is_live`. Slider changes, typed time, and LIVE snapping now cross
  the existing `SimCommand` queue; `SetTime` and `SnapToLive` also round-trip
  through the deterministic replay format. Transition-only `TickReport`s from
  both commands and ticks are bridged to auto-expiring `ui_kit` toasts without
  re-deriving clock levels. Five regressions cover every slider detent, invalid
  date/time edits, all four LIVE regimes, synthetic toast transition counts,
  and replay/clamp reporting, raising the workspace from 114 to 119 tests. An
  exact-app capture verified the assembled bar and the corrected diagnostic
  placement; the final 90-frame native smoke rendered at 115.2 fps after its
  warmup. Evidence: `cargo test`, `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, all-target checks,
  `git diff --check`, and `xtask gen-catalog --dry-run` pass; no dependencies
  or generated catalog assets changed.
- **2026-07-13** — Started WP8 from the green 114-test WP7 checkout after
  reading ARCHITECTURE §§4.2, 7, and 9.5–9.6. The time bar will bind WP1's
  `SimClock`, `RateIndex`, strict parsers, LIVE predicate, and transition-only
  `TickReport` directly through the existing `SimCommand` mutation boundary;
  no time math, level re-derivation, dependencies, or WP9+ behavior is added.
- **2026-07-13** — WP7 done. Added the call-site-stable `ui_kit` façade and
  `UiKitPlugin`: snapshotted dark theme tokens, wide-tracked Inter typography,
  seven code-defined BSN scene functions, a navigation-stack-bound top bar and
  breadcrumb, and an `EditableText` search placeholder whose behavior remains
  with WP12. Debug builds spawn a scrollable 28-cell gallery covering all seven
  widgets in default/hover/active/disabled states. Its real resolved scene test
  verifies a non-empty `AccessibleLabel` and generated `AccessibilityNode` on
  every widget root. Inter 4.1, the upstream SIL OFL 1.1 text, source URL, and
  SHA-256 audit metadata are vendored together under `assets/fonts`; the Bevy
  asset root is explicitly aligned with the workspace assets directory. A
  2560×1440 exact-app capture verified the themed gallery and HUD, and the final
  80-frame native smoke gate measured 120.1 fps. Evidence: `cargo test` 114/114,
  `cargo clippy --workspace --all-targets -- -D warnings`, release all-targets
  check, formatting/diff checks, and `xtask gen-catalog --dry-run` all pass.
- **2026-07-13** — Started WP7 after a green 108-test workspace baseline and
  reading ARCHITECTURE §§8.4, 9, and 9.1. The implementation stays on Bevy
  0.19's code-defined BSN scene path with a call-site-stable `ui_kit` façade,
  AccessKit labels, an SIL-OFL font plus audit metadata, the top bar and
  breadcrumb model, and a debug-only all-state widget gallery. WP8/WP10–WP12
  behavior remains out of scope.
- **2026-07-13** — WP6 done. Added `OrbitLinesPlugin` with 65 retained orbit
  paths whose f64 vertices remain parent-relative while each line entity is
  independently rebased around the camera focus. Ellipses use 256–768 samples
  by eccentricity with uniform true-anomaly spacing and a bit-identical seam;
  the Nereid regression requires perihelion chords below 20% of its apoapsis
  spacing. `Elements::is_hyperbolic` selects a 767-vertex open 3I/ATLAS branch
  with an exact perihelion center sample and endpoints at ±25 Julian years.
  Sampling uses `elements_at` at the current simulation time plus the same
  fitted/two-body mean motion and `state_from_elements` path as propagation.
  Added the eight-planet color LUT, shared per-category defaults, quantized
  camera-distance/view-angle alpha fades, and an `OrbitLineBrightness`
  resource hook for WP13. A small negative retained-line depth bias prevents
  body/path z-fighting; the full-system re-anchor test holds parent vertices
  unchanged across Mercury and Sedna focus origins. Eleven new tests cover
  every required sampler/property gate plus colors, fades, all-65 spawning,
  anchoring, and invalid-input rejection. Evidence: `cargo test` passed
  108/108; fmt, warning-denied clippy, `git diff --check`, and a warning-denied
  release build passed; a 600-frame release GPU smoke focused on Mercury
  measured 60.0 fps with every orbit line
  enabled. The known forced-exit winit warning remains non-gating. No
  dependency or generated-data changes.
- **2026-07-13** — Started WP6 after a green 97-test workspace baseline and
  reading ARCHITECTURE invariant 6 plus §10.2. The implementation is scoped
  to parent-relative adaptive orbit sampling, the open 3I/ATLAS ±25-year arc,
  color/fade rendering, floating-origin re-anchoring, and the required tests;
  WP10/WP11 toggles and WP13 emphasis behavior remain out of scope.
- **2026-07-13** — WP5 done. The human confirmed completion after commit
  `6095dab` (`feat: WP5`) was pushed to `origin/main`; this closes the hosted
  macOS + Windows replay-determinism acceptance item and the dashboard status.
  Both OS jobs execute the same 600-frame, 500+-mixed-command replay test and
  assert the pinned canonical f64-state hash `12568395442970282829`. The
  implementation commit contains the complete 97-test local gate evidence
  recorded below; this entry is the final human-confirmed hosted-CI evidence.
- **2026-07-13** — WP5 implementation and local acceptance are complete;
  the dashboard remains in-progress only until the pinned replay test has
  passed a hosted macOS + Windows CI run. Added a sole raw-input intent
  module and table-driven key map; private f64 camera/control state with one
  grep-pinned `SimCommand` mutation gate; an orbit/focus parent rig with the
  1.2× body-radius to 1.5× Sedna-aphelion dolly range; interruptible eased
  travel that tracks the target's current propagated position and lands into
  exact Follow; and lossless frame/TDB-time-stamped replay serialization.
  The headless harness reuses the desktop command, clock, propagation, and
  tween paths and pins the canonical cross-platform f64-state hash
  `12568395442970282829` over 600 frames and more than 500 mixed commands;
  render/f32 state is excluded. Eight new tests cover the input boundary and
  key table, moving-Io convergence/follow plus interruption, both zoom clamps,
  camera parenting/clip planes, corrupt replay/catalog rejection, timestamps,
  and record → serialize → parse → replay equality. Evidence: `cargo test` and
  nextest passed 97/97 with zero skips; fmt, clippy `-D warnings`, diff checks,
  and warning-denied release library/binary builds passed. Clean 600-frame
  release smokes measured 80.9 fps focused on Mercury and 83.7 fps on Sedna;
  the known forced-exit winit teardown warning remains non-gating. No
  dependency or generated-data changes.
- **2026-07-13** — Started WP5 after reading ARCHITECTURE §§3, 8.2, 8.3,
  and 12. The implementation keeps raw device input in one intent module,
  routes every semantic action through the single `SimCommand` consumer,
  and extends the f64 simulation path with an interruptible moving-target
  camera tween plus a serialized headless replay/hash gate.
- **2026-07-13** — WP4 done. Added the testable `solar-sim` library with
  validated real-catalog startup loading, a user-facing failure screen, f64
  heliocentric state storage, ordered parent/moon composition through
  `kepler::state_at`, 1,000 km/unit camera-focus rebasing, 66 colored
  true-radius spheres, emissive Sun placeholder, +REAL clock driving, and a
  command-queued frame flow pinned as input → commands → clock → propagation
  → origin → render. Seven new tests prove all eight planets match direct
  `sim-core` output bit-for-bit, Io composition is bit-identical, Mercury and
  Sedna focus points rebase exactly, relative positions survive focus changes,
  corrupt input produces an error screen without panic, all 66 spheres spawn,
  and system sets execute in contract order. Evidence: `cargo test` 89 passed;
  nextest 89 passed with zero skips; fmt and clippy clean; warning-denied
  release lib + binary builds passed; dev-feature smoke passed at 117.5 fps;
  missing-catalog GUI smoke exited 0; warmed 600-frame release runs measured
  60.2 fps focused on Mercury and 115.5 fps focused on Sedna. The known
  Bevy/winit forced-exit teardown warning remains non-gating. No dependencies
  were added.
- **2026-07-13** — Normalized the completed WP0/WP3 status: WP0 is now
  explicitly labeled acceptance-complete, while the real-Windows launch is
  isolated as a deferred pre-WP16 release gate rather than an open WP0 item.
  Started WP4 after reading ARCHITECTURE §§3, 4.3, 8.2, 8.3 and the nested
  `sim-core` contract; dashboard status is now in-progress.
- **2026-07-13** — Finalized `docs/wp0-dev-setup-macos.md` as the completed
  WP0/WP3 setup and regeneration record. Marked every section complete,
  removed stale pre-commit language, aligned the diagnostics-import example
  with the warning-clean source, and replaced the historical 72-test note
  with the current 82-test baseline. The real-Windows launch remains the
  explicitly deferred pre-WP16 release gate, not unfinished WP0/WP3 work.
  Evidence: `cargo test` 82 passed; fmt, clippy, and diff checks passed.
- **2026-07-13** — WP3 done. Commit `1ea4d1f` (`feat: Q7 and WP3`) is on
  `origin/main` with `assets/catalog.ron`, all 68 captured JPL responses,
  the two active spot-check files, the approved DE440 GM set, and its tests
  and documentation. The post-commit worktree was clean. This satisfies the
  final online-capture/artifact-commit acceptance item; WP3 is now ✅ done.
- **2026-07-13** — Human approved Q7's complete JPL DE440 Sun/planet GM
  table. Applied eight replacements (Venus was already exact), added emitted
  DE440 provenance, cleared the final `TODO(review)`, added a nine-body
  regression test, and regenerated `assets/catalog.ron` plus the spot-check
  catalog from the captured responses through `xtask`. Evidence: `cargo test`
  82 passed; nextest 82 passed with zero skips; online-feature xtask suites 30
  passed; active `horizons_position_spot_check` passed; fmt, clippy,
  warning-denied release build, and diff check passed; normal and `--features
  dev` macOS launches each rendered 60 smoke frames and exited 0. The expected
  Bevy/winit teardown warning remains non-gating. The curated review item is
  complete; WP3 remains in-progress only until its generated and captured
  artifacts are committed.
- **2026-07-13** — Audited the final open WP3 curated-data marker against
  JPL DE440 and recorded the exact nine-row decision table in
  `docs/wp3-gm-audit-2026-07-13.md`. Eight constants differ; Venus is already
  exact. Opened Q7 for the required human sign-off. No GM, generated catalog,
  truth fixture, or review marker was changed pending that decision.
- **2026-07-13** — Re-audited the complete
  `docs/wp0-dev-setup-macos.md` procedure against the current checkout and
  refreshed its stale A6/B4 status. Found and fixed one release-only warning
  by cfg-gating the debug diagnostics imports in `solar-sim`; a
  warning-denied release build is now clean. Current evidence: Xcode CLT
  present; rustc/cargo 1.95.0; cargo-nextest 0.9.140; Bevy 0.19.0 locked;
  `cargo test` and `cargo nextest run --workspace` each passed 81/81 with
  zero skips; fmt and clippy clean; isolated offline Rust 1.75 `sim-core`
  check passed; core-purity/default-offline/CI-feature checks passed; the
  six-body fixture pipeline passed; normal and `--features dev` macOS smoke
  launches each rendered 60 frames and exited 0. Hosted Windows/macOS
  evidence remains [Actions run #3](https://github.com/jiayanzeng/solar-sim-workspace/actions)
  for commit `5540cdd`, with lint, test-macos, build-windows, and invariants
  successful. WP0 remains ✅ done; real-Windows hardware launch remains the
  intentional pre-WP16 deferral.
- **2026-07-13** — Human explicitly approved the calibrated spot-check
  budgets: planets 1 km, moons 10 km, dwarf planets 25,000 km, and comets
  30,000,000 km. This closes the remaining approval ambiguity in step 5;
  the already-green active gate and its fixture values are unchanged.
- **2026-07-13** — Human closed Q6 with the recommended spot-check epoch
  policy. Activated all 10 bodies at JD 2461042.0 plus Halley at JD
  2446471.0, while retaining the other nine historical vectors as
  `gate: false` audit data. The test pins 20 captured records, 10 distinct
  active ids, the Halley-only historical gate, and ordered per-category
  budgets: planets 1 km, moons 10 km, dwarf planets 25,000 km, comets
  30,000,000 km. Measured maxima are documented in the WP3 spec; the
  approved unwrapped-MA, coarse least-squares, and SBDB normalization
  contracts are unchanged. Evidence: active spot-check passed with 11
  points; `cargo test` 81 passed; online-feature xtask suites 29 passed;
  fmt clean; clippy zero warnings; `git diff --check` clean.
- **2026-07-13** — Transcribed the human-captured Horizons VECTORS truth
  into `xtask/fixtures/spotcheck/vectors.json`: 20 parent-centric,
  ecliptic-J2000 positions covering the specified 10 bodies at JD
  2461042.0 and 2446471.0. Halley's ambiguous designation response was
  resolved to Horizons record 90000030 (1P/Halley, JPL#75) and recaptured.
  Zero-tolerance calibration deliberately failed: catalog-epoch matches
  are exact or close, but historical phase errors reach 144,450,813.9 km
  for Earth, 14,762,084.4 km for Phoebe, and 23,021,717.1 km for 3I/ATLAS.
  Q6 records the contract conflict; no permissive tolerances were invented,
  the spot-check checklist remains open, and the workspace gate is
  intentionally red pending human direction.
- **2026-07-13** — Human approved all eight flagged radius changes from
  `docs/wp3-radius-audit-2026-07-13.md`. Updated Himalia 75→85 km, Nix
  19.5→18 km, Hydra 18→18.5 km, Hiʻiaka 160→185 km, Namaka 85→75 km,
  Hygiea 217→203.56 km, 67P 2.0→1.7 km, and Hartley 2 0.6→0.8 km;
  attached individual physical provenance, cleared the radius review
  marker, and regenerated the 66-body catalog through captured-fixture
  replay. A manifest regression test pins every approved value and source.
  Evidence: `cargo test` 81 passed; online-feature xtask suites 29 passed;
  `cargo fmt --all -- --check` clean; clippy zero warnings; generated
  catalog inspection confirmed all eight values; `git diff --check` clean.
- **2026-07-13** — Completed the source-backed 66-body radius audit in
  `docs/wp3-radius-audit-2026-07-13.md`, using current JPL planetary and
  satellite tables, live SBDB `phys-par=true` responses, and primary
  literature where JPL has no current radius. All 66 ids occur exactly
  once: 58 are within the 2% review threshold or already approved; eight
  proposed updates (Himalia, Nix, Hydra, Hiʻiaka, Namaka, Hygiea, 67P,
  Hartley 2) await human sign-off. No flagged manifest value was changed.
- **2026-07-13** — Regenerated the 66-body catalog from the 68 captured
  JPL payloads after the Q2/Q3 manifest changes. The emitted Pluto record
  contains GM 975.5 and the explicit 869.6 + 105.9 Pluto+Charon
  provenance; the emitted 3I/ATLAS record contains radius 0.5 km and the
  approved HST/NGA provenance. Evidence: `cargo test` 81 passed;
  online-feature xtask suites 29 passed; fmt clean; clippy zero warnings;
  `git diff --check` clean.
- **2026-07-13** — Human closed Q2/Q3. The curated manifest now uses
  Pluto+Charon system GM 975.5 km³/s² (869.6 + 105.9) for Pluto-system
  moon propagation and an adopted 0.5 km nucleus radius for 3I/ATLAS.
  Both emitted source strings carry the approved derivation/citations,
  and a manifest regression test pins the values and provenance. The
  broader 66-body radius audit remains open. Catalog regeneration and
  verification follow this entry.
- **2026-07-13** — The full WP3 online generation succeeded: 66 bodies
  emitted to `assets/catalog.ron` and 68 exact responses captured under
  `xtask/fixtures/captured-2026-07` (65 body fetches + three TNO lookup
  payloads). The first complete fetch exposed SBDB `null` values on
  unrelated 3I/ATLAS fields (`per`, `ad`); the parser now filters to the
  eight consumed orbital fields before numeric conversion, with an
  adversarial regression test. Fixture replay reproduced every catalog
  datum exactly; only the command/timestamp provenance header differed.
  Evidence: `cargo test` 80 passed; online-feature xtask suites 28 passed;
  fmt clean; clippy zero warnings; `git diff --check` clean. The artifacts
  await an explicit commit, so the online-capture checklist remains open.
- **2026-07-13** — Implemented approved Q5 routing after the human-authored
  ARCHITECTURE §5.3 edit: Mercury–Mars remain on geometric center commands
  199/299/399/499; Jupiter–Neptune now use barycenter commands 5/6/7/8.
  Added a manifest regression test pinning the complete eight-planet split
  and updated the WP3 spec/setup guide without changing the unwrapped-MA,
  least-squares secular-fit, or SBDB normalization contracts. Verification
  and the full online capture follow this entry.
- **2026-07-13** — Q5 closed by human decision: keep Mercury–Mars on
  geometric planet centers and switch Jupiter–Neptune to system
  barycenters for the 1800–2300 fit. The decision explicitly preserves
  the unwrapped-MA near-pair slope, coarse-span least-squares secular fit,
  and all SBDB AU→km / `q/(1−e)` / perihelion-rebasing rules. Per the
  repository's human-maintained-file rule, route implementation waits for
  the corresponding human edit to ARCHITECTURE §5.3.
- **2026-07-13** — WP3 capture prerequisites hardened without changing
  Q5-controlled planet routes. Added `--capture DIR`, exact raw-response
  dumps on Horizons parse failure, and strict Horizons Lookup API 1.1
  resolution for Dysnomia / Hiʻiaka / Namaka plus their parent-primary
  centers. Live evidence: the pre-Q5 online run captured Mercury through
  Jupiter and preserved Jupiter's exact post-2200 error; JPL parent-system
  lookups returned unique SPK IDs whose three parent-centric ELEMENTS
  probes all returned `$$SOE` at JD 2461042. Tests: `cargo test` (78
  passed), `cargo test -p xtask --features online` (26 passed across the
  xtask suites), fmt clean, clippy zero warnings. Q5 remains blocked on the
  human-owned ARCHITECTURE §5.3 route wording/sign-off.
- **2026-07-13** — WP0 done. GitHub Actions `ci` run #3 for commit
  `5540cdd` completed successfully: `lint`, `test-macos`,
  `build-windows`, and `invariants` all passed. The accompanying local
  rerun had 72/72 workspace tests green, and the catalog dry-run plus
  fixture regeneration completed successfully. The four hosted-run
  annotations are the non-gating Node.js 20 deprecation emitted by
  `actions/checkout@v4`, not Rust/clippy warnings; upgrading to the
  current `actions/checkout@v7` is follow-up maintenance and does not
  reopen WP0. Real-Windows hardware launch remains explicitly deferred
  to WP16.
- **2026-07-13** — WP0 local close-out gates completed; hosted CI remains.
  Corrected the Bevy 0.19 shell to `MessageReader` / `MessageWriter`, made
  the J2000−Unix derivation load-bearing with its promised regression test,
  formatted the workspace for the new CI gate, and hardened `ci.yml` with
  exact Rust 1.95/1.75 toolchains plus non-self-matching purity/offline
  checks. Evidence: `cargo test` and `cargo nextest run --workspace`
  (72 passed, 0 failed/skipped); `cargo fmt --all -- --check`;
  `cargo clippy --workspace --all-targets -- -D warnings`; isolated
  `cargo +1.75.0 check` for `sim-core`; fixture/purity/offline checks; and
  normal + `--features dev` macOS launches rendering 60 smoke frames and
  exiting 0. WP0 acceptance remains open pending the hosted macOS/Windows
  workflow; real-Windows launch remains deferred to WP16.
- **2026-07-12** — Q1 closed by human direction after the toolchain pin
  landed: Bevy 0.19.0 declares Rust 1.95.0 on crates.io;
  `rust-toolchain.toml` pins 1.95.0 and `sim-core` retains its independent
  `rust-version = "1.75"` claim. Evidence:
  `docs/open-questions-brief-2026-07-12.md` §Q1 and commit `61896e8`.
- **2026-07-12** — TASKS.md revision: added detailed Work package briefs
  (WP4–WP18) with human-owned acceptance criteria; recorded Q1 answer
  (Bevy 0.19.0 MSRV 1.95.0, crates.io evidence); raised Q5 (Horizons
  giant-planet route failure — online run failed at Jupiter with
  `no $$SOE`; diagnosis + proposal in
  `docs/open-questions-brief-2026-07-12.md`); WP3 online-capture item
  marked `blocked(Q5)`; noted expected baseline 71→72 with the
  `UNIX_EPOCH_JD` warm-up test. Evidence: 2026-07-12 `cargo test` run
  (71 passing) and the failed `--online` run transcript.
- **2026-07-12** — Project organization: ARCHITECTURE.md Rev C created
  (read-only for agents), this file created, AGENTS.md (root + nested)
  created. Evidence: repo tree; `cargo test` 71 passing.
- **2026-07-12** — WP2 done: `kepler.rs` +14 tests (sweeps, invariants,
  RK4 cross-check, retrograde + Nereid fixtures); spot-check harness armed
  (`xtask/tests/spotcheck.rs`). Evidence: `cargo test` 71 passing.
- **2026-07-12** — WP1 done: `time.rs` +21 tests. Fixed en route: exact
  integer-second calendar path (fractional-JD detour lost ~10 µs);
  J2000−Unix constant is 946 728 000 (noon), not 946 684 800 (midnight).
  Evidence: `cargo test` 56 passing at the time.
- **2026-07-12** — WP3 core done: schema + validation + generator +
  fixtures + smoke + spec. Manifest ordering bug caught by its own test
  (TNO moons before dwarf parents) and fixed. Evidence: `cargo test` 35
  passing at the time; `assets/catalog.sample.ron` emitted and reloaded.
