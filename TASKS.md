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
| 16 | Steam: Steamworks init, overlay spike, packaging/signing/depots | deferred |
| 17 | QA: replay suite, perf gates, demo script, licensing audit | todo |
| 18 | *Optional:* Compare Size mode | deferred |

**Test baseline: 223 passing** (53 `sim-core` · 119 `solar-sim` · 48 `xtask`
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
- [x] Default (non-`steam`) build has no Steamworks in its dependency
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

The completed stabilization cycle has a human-requested architecture-
conformance follow-up. Execution is awaiting review and authorization of
`docs/ui-gameplay-architecture-conformance-2026-07-17.md`; no work package is
currently reopened by this queue.

1. [ ] **Hyperbolic orbital-period omission — justified.** Deferred
   documentation clarification only; no immediate source action. WP10
   explicitly derives period from `Orbit::period_s` and permits hyperbolic
   bodies to show no period.
2. [x] **AC-1 — command-path non-compliance.** Route Layers-panel open/close
   through explicit desired-state `SimCommand` traffic and shared
   desktop/headless reducers. Coordinate under WP11.
3. [x] **AC-2 — plugin-graph non-compliance.** Restore the architecture-facing
   §8.2 plugin names, responsibilities, and frame ownership without duplicating
   existing internal systems. Coordinate under WP4.
4. [ ] **AC-3 — top-bar order non-compliance.** Restore Search-before-Menu
   visual and keyboard order with responsive/accessibility regressions.
   Coordinate under WP7.
5. After the conformance phases are accepted and submitted in order, continue
   with the still-deferred WP16 and dependent WP17 only under their existing
   human authorization and hardware gates.

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

### Q10 — CLOSED (human, 2026-07-14).

WP15's "stable across two consecutive CI runs
on the same platform" is scoped to a SINGLE platform, and that platform is
macOS/Metal. Rationale: hosted windows-latest has no GPU, so wgpu falls back to
the WARP software rasterizer. Proving WARP is deterministic across two runs is
near-tautological and tests nothing the Metal run does not. Real-GPU DX12 golden
validation is deferred to WP16/WP17 bring-up on real hardware. DX12 captures stay
in the golden workflow as a non-blocking code-path check. No acceptance text is
reworded; this records how the existing text is read.

### Q11 — CLOSED (human, 2026-07-14).

Yes, create the platform matrix. Docker is not
required: the repository is public, so GitHub-hosted standard runners are free
and unlimited. Validate the Linux lane on hosted CI by pushing a branch and
reading the run. Self-hosted runners and local VMs are OFF the table.

### Q12 — OPEN.

The 2026-07-14 CI follow-up instructs agents to work tasks CI-1
through CI-6 in order, but does not define the scope, acceptance evidence, or
commands for any of those six tasks. Provide the exact CI-1 through CI-6 briefs;
agents must not infer them from the superseded private-repository Task 1/Task 2
numbering.

### Q13 — OPEN.

Hosted-only CI (the standing decision as of 2026-07-14: no
self-hosted runners, no local VMs) cannot satisfy three acceptance items. They
require physical hardware and a human purchasing decision:

1. WP16 — "Overlay spike results documented in docs/ for both OSes."
   Needs a real Steam client on a real desktop session on each OS.
2. WP16 — "a dev-branch SteamPipe install launches on both OSes."
   Hosted windows-latest has no GPU (WARP fallback); that is not a launch
   verification. The macOS half is satisfiable on the developer's own Mac.
3. WP17 — "Perf numbers recorded for both reference machines; both >= 60 fps
   all-layers", on an M1 MacBook Air and a GTX 1650-class laptop. No hosted
   runner can produce a credible frame-time measurement, and neither reference
   machine is currently owned.

Decision required from the human, before WP16 packaging begins: acquire the
reference hardware, or amend WP17's reference machines with a signed change.

WP16 will need Apple Developer ID and Steam credentials as repository secrets.
On a public repo, those MUST live in a protected environment that no
fork-triggered workflow can reach.

Human partial ruling (2026-07-16): proceed Mac-first with the M2 Pro real-client
overlay check against interim App ID 480. The Windows overlay spike,
both-platform dev-branch install evidence, and WP17 reference hardware remain
deferred to the existing "before packaging begins" decision deadline. This
narrows Q13 but leaves its hardware-purchase half open; no affected acceptance
criterion is closed by the ruling.

### Q14 — CLOSED (human, 2026-07-16).

Approved `steamworks = "0.13.1"` as an optional `crates/solar-sim` dependency
behind the `steam` feature. App ID 480 (Valve's public Spacewar SDK test app) is
the committed INTERIM development ID, with one provenance-commented constant,
no fallback, a pinning unit test, generated-and-gitignored `steam_appid.txt`, and
hard package/depot refusal while the value remains 480. The real App ID requires
a new Open question after the Steamworks partner account exists; the approved
swap is the constant, its pinning test, and regeneration of `steam_appid.txt`.
Full rationale and human sign-off:
`docs/wp16-steam-bringup-decisions-2026-07-15.md`.

### Q15 — CLOSED (human, 2026-07-16).

The 2026-07-16 M2 Pro overlay attempt exposed a WP14 recovery ambiguity outside
WP16. The persisted `settings.toml` has `orbits`, `labels`, and `icons` set to
false; combined with the non-persisted default ×1 body scale and full-system
camera, the human reports that no bodies or orbits are discoverable. The human
also reports that opening Settings hangs the application, while the exact
60-frame Metal smoke with `--assert-nonblack` exits 0 at 202.7 fps. Decision
required: add an explicit reset-settings recovery path, impose a minimum
startup visual-cue floor, or preserve exact persistence and document a manual
reset.

Human validation on 2026-07-16 narrows this question: at commit
`60a19a6718edbc3b239606325f1b663c723d5a12`, the real macOS release build's
Settings modal accepted pointer adjustments and scrolling, and `REVERT`,
`APPLY`, `CLOSE`, and Escape all worked. Hosted
[run 29488349896](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29488349896)
passed all five jobs for that commit. The modal defect is resolved.

Human ruling on 2026-07-16: lock in both compatibility measures. Provide an
explicit Settings `RESTORE DEFAULTS` action plus a `--reset-settings` startup
path, and impose a minimum discoverability floor without silently overriding
persisted values. The implemented floor is a transient recovery notice shown
only when User Interface is on while Orbits, Labels, and Icons are all off; it
queues `RestorePresentationDefaults`, does not write settings merely by
appearing, and yields to the existing `SHOW UI` affordance when User Interface
is off. Exact WP14 layer persistence remains intact. The integrated design,
task order, and acceptance evidence are recorded in
`docs/ui-gameplay-stabilization-2026-07-16.md`.

### Q16 — CLOSED 2026-07-17.

Stabilization Task 6 says Saturn's sphere, rings, label, and icon must share
one orbit-emphasis blend, but ARCHITECTURE §10.3 deliberately makes the Sun and
all planets text-only and reserves circular Icons-layer reticles for
"everything else." The implementation and its 57-reticle invariant enforce
that design, so no Saturn icon exists to fade. Should Task 6 be read
architecture-preservingly as Saturn's complete actual aggregate
(sphere/rings/text label) plus a representative icon-bearing fast body such as
Io, or is a planet icon an intended architecture revision? Recommendation:
preserve Rev C, keep Saturn text-only, and use Io to prove the shared
Icons-layer reticle blend. Agents must not add a Saturn reticle without a human
ruling and corresponding architecture update.

Human ruling on 2026-07-17: preserve ARCHITECTURE §10.3 and Rev C without an
architecture revision. Saturn's complete render aggregate is its sphere,
rings, text label, and orbit; the Sun and all planets remain strictly
text-only, so Saturn MUST NOT gain an icon or reticle. Io is the representative
icon-bearing fast body used to prove the shared Icons-layer reticle blend.
Accordingly, stabilization Task 6's aggregate acceptance is satisfied by
Saturn's complete architecture-valid aggregate plus Io's reticle coverage.

## Change log (append-only; newest first)

- **2026-07-17** — Completed architecture-conformance Phase 2 and returned
  WP4 to **✅ done**. Application assembly now exposes the exact
  architecture-facing owners `TimePlugin`, `PropagationPlugin`,
  `OriginPlugin`, `CameraPlugin`, `LabelsPlugin`, `ScenePlugin`,
  `OrbitLinesPlugin`, `SelectionPlugin`, `UiKit`, `HudPlugin`,
  `SearchMenuPlugin`, `SettingsUiPlugin`, and `PlatformPlugin`; feature-gated
  `SteamPlugin` retains its required pre-graphics initialization. Focused
  helpers are composed once beneath those owners: Camera owns raw-input
  translation and camera startup, Scene owns bodies/polish/starfield, HUD owns
  layers/time/left-panel surfaces, Settings UI owns schema/modal/persistence,
  and Platform owns window/runtime settings, renderer recovery, and frame/exit
  lifecycle. The chained input → commands → clock → propagation → origin →
  camera → render sets, f64 propagation, sole rebase boundary, settings
  bootstrap, golden capture, and no-op/Steam service boundary are unchanged.

  One new assembly regression pins all 13 owners in §8.2 order, rejects
  duplicate owners, proves each internal helper has exactly one composition
  owner, and checks required resources. Existing independent tests continue to
  prove one 66-body scene, one camera, 66 labels with the architecture-correct
  57 reticles, 65 orbits, retained HUD/settings surfaces, and exact frame flow.
  The portable replay hash remains bit-identical at
  `8282160698094571922`; Q15 recovery, Q16's text-only Saturn plus Io reticle,
  golden definitions, and responsive UI behavior remain unchanged.

  `cargo test` passes **336 tests** (53 `sim-core` · 232 `solar-sim` · 48
  `xtask` lib · 2 xtask smoke · 1 active spot-check), and `cargo test -p
  solar-sim --features steam` passes **233 tests**. `cargo fmt --all --
  --check`, both zero-warning clippy configurations,
  `scripts/check-texture-metadata.sh` (16 assets), and `git diff --check` pass;
  `cargo tree -p solar-sim --no-default-features --edges normal --prefix none`
  contains no Steamworks package. No dependency, deferred WP16 implementation,
  catalog/generated/truth asset, architecture/agent file, numerical tolerance,
  or AC-3 top-bar source changed.
- **2026-07-17** — Reopened WP4 for authorized architecture-conformance
  Phase 2 after Phase 1 commit `21352b9` passed its complete gate and was
  pushed. The mandatory pre-code review re-read ARCHITECTURE invariants 4 and
  6–8 and §§8.2–8.5 and 12, the WP4 brief, `TASKS.md`, and the complete
  corrective plan. The current responsibility map is: root
  `apply_sim_commands`/`tick_clock` own command-to-time flow;
  `PropagationPlugin` owns f64 body truth; `OriginPlugin` owns focus advance
  and the sole f64→f32 rebase; `CameraRigPlugin` plus the internal
  `InputIntentPlugin` and root camera startup own input/camera work;
  `LabelsPlugin` owns projection/declutter while `SelectionPlugin` owns the
  viewport pick surface; root sphere startup plus `ScenePolishPlugin` and
  `StarfieldPlugin` own bodies, Sun light/bloom, emphasis, and starfield;
  `OrbitLinesPlugin` owns retained parent-relative paths; `UiKitPlugin`
  currently mixes theme/gallery with top-bar/breadcrumb HUD work;
  `LayersPlugin`, `TimeBarPlugin`, and `LeftPanelPlugin` own the remaining HUD;
  `SearchPlugin` owns Search/Browse; `ProductSettingsPlugin` currently mixes
  settings UI/persistence with window/runtime settings and renderer recovery;
  `PlatformServicesPlugin` owns the no-op/Steam service lifecycle while root
  systems own smoke/frame lifecycle; feature-gated `SteamPlugin` correctly
  initializes before `DefaultPlugins`.

  The architecture-preserving correction will expose exactly the §8.2 owners:
  `TimePlugin`, `PropagationPlugin`, `OriginPlugin`, `CameraPlugin`,
  `LabelsPlugin`, `ScenePlugin`, `OrbitLinesPlugin`, `SelectionPlugin`,
  `UiKit`, `HudPlugin`, `SearchMenuPlugin`, `SettingsUiPlugin`,
  `PlatformPlugin`, and feature-gated `SteamPlugin`. Focused implementations
  remain private subplugins, each system will have one composition owner, and
  the existing frame-set chain remains unchanged. `SteamPlugin`/the no-op
  service boundary stays before graphics creation; settings bootstrap stays
  before clock/layer derivation; Platform takes window/runtime and render
  recovery while Settings UI retains schema, modal, and persistence ownership.
  Assembly regressions will pin exact owner order, one instance of every
  application-facing plugin, required resources, and the existing independent
  66-body/camera/label/orbit/HUD startup invariants. No behavior, replay hash,
  golden definition, dependency, deferred WP16 implementation, generated/truth
  asset, numerical tolerance, or Q15/Q16 ruling may change.
- **2026-07-17** — Completed architecture-conformance Phase 1 and returned
  WP11 to **✅ done**. Layers-panel visibility is now canonical
  `PresentationState` reduced from explicit desired-state
  `SimCommand::SetLayersPanelOpen(bool)` traffic shared by desktop and
  headless execution. The rail observer reads canonical state only to enqueue
  one command; it no longer mutates panel state or a desktop-only dirty flag.
  Replay-v2 serializes/parses the new command, rejects invalid booleans and
  field counts, and hashes panel visibility as part of complete presentation
  identity; replay-v1 remains parseable. The portable replay hash intentionally
  changes from `1535747298578131566` to `8282160698094571922` solely because
  this previously omitted canonical state is now covered. Duplicate desired
  states advertise no false Bevy change, ordered open/close sequences converge
  identically across the real desktop gate and headless runner, and a static
  regression pins the observer's command-only boundary. Composed UI coverage
  proves necessary panel rebuilds retain rail/panel scroll and semantic focus;
  existing UI-off, Browse/Settings modal priority, stable entity identity, and
  responsive reachability regressions remain green.

  Four new app regressions raise `cargo test` to **335 tests** (53 `sim-core`
  · 231 `solar-sim` · 48 `xtask` lib · 2 xtask smoke · 1 active spot-check);
  `cargo test -p solar-sim --features steam` passes **232 tests**. `cargo fmt
  --all -- --check`, both zero-warning clippy configurations,
  `scripts/check-texture-metadata.sh` (16 assets), and `git diff --check` pass.
  No WP4/WP7 source, deferred WP16 implementation, dependency,
  catalog/generated/truth asset, architecture/agent file, numerical tolerance,
  or Q16 Saturn/Io behavior changed.
- **2026-07-17** — Reopened WP11 for authorized architecture-conformance
  Phase 1 after the documentation baseline was committed as `cf7aab1` and
  pushed to `codex/ui-gameplay-remediation`. The mandatory pre-code review
  re-read ARCHITECTURE invariants 4 and 7 and §§8.2, 9.3–9.4, and 12,
  `TASKS.md`, the completed stabilization record, and the complete corrective
  plan. It confirms that `RailAction::ToggleLayersPanel` directly mutates the
  desktop-only `RailUiState.layers_panel_open`, so the user-visible transition
  is absent from replay, combined state hashing, and headless convergence.
  The correction will add an explicit desired-state command, reduce it through
  the shared canonical presentation state, and make the rail observer enqueue
  exactly one command without mutating that state. Replay-v2 serialization,
  strict parsing, corrupt-input rejection, portable hashing, duplicate-command
  idempotence, and ordered desktop/headless parity will be extended together;
  replay v1 remains accepted. Retained panel identity, scroll/focus recovery,
  UI-off's sole `SHOW UI` affordance, and Browse/Settings modal precedence must
  remain unchanged. Targeted static and composed regressions will precede the
  full normal/Steam submission matrix. No WP4/WP7 source, deferred WP16 work,
  dependency, catalog/generated/truth asset, architecture file, numerical
  tolerance, or Q16 Saturn/Io behavior is in scope.
- **2026-07-17** — Recorded the human-requested, architecture-governed
  conformance queue without changing source code or work-package status.
  `docs/ui-gameplay-architecture-conformance-2026-07-17.md` classifies WP10's
  hyperbolic no-period behavior as a justified deferred clarification and
  records three unwaived violations: Layers-panel visibility bypassing
  `SimCommand`, the missing ARCHITECTURE §8.2 application-facing plugin graph,
  and Menu preceding Search in the top bar. The plan orders remediation under
  WP11 → WP4 → WP7, one work package at a time, with targeted acceptance,
  complete normal/Steam verification, and automatic commit/push only after the
  human authorizes the plan and each phase's evidence confirms completion.
  This documentation-only planning step does not resume deferred WP16, alter
  Q16's Saturn/Io ruling, or edit acceptance criteria, generated/truth assets,
  dependencies, `ARCHITECTURE.md`, or any `AGENTS.md`.
- **2026-07-17** — Synchronized the UI/gameplay stabilization completion
  record after the human-requested documentation audit; no source code or
  work-package status changed. `docs/ui-gameplay-stabilization-2026-07-16.md`
  now records the architecture-preserving Q16 result (Saturn's
  sphere/rings/text/orbit aggregate with no Saturn icon or reticle, plus Io
  reticle coverage), the complete Task 7 retained-update and composed-lifecycle
  outcome, the final portable replay hash `1535747298578131566`, and the final
  verification totals instead of the earlier intermediate checkpoint. Fresh
  post-update evidence is green: `cargo test` passes **331 tests** (53
  `sim-core` · 227 `solar-sim` · 48 `xtask` lib · 2 xtask smoke · 1 active
  spot-check); `cargo test -p solar-sim --features steam` passes **228 tests**;
  formatting, both zero-warning clippy configurations, and `git diff --check`
  pass. The locked Task 6 acceptance text and append-only historical entries
  remain unchanged.
- **2026-07-17** — Human explicitly closed Q16 with the recommended
  architecture-preserving ruling and WP13 returned to **✅ done**. Per
  ARCHITECTURE §10.3, Saturn remains strictly text-only: its complete aggregate
  is the sphere, rings, text label, and orbit, with no Saturn icon or reticle;
  Io supplies the representative Icons-layer reticle coverage for the shared
  high-rate blend. This resolves the only remaining ambiguity in stabilization
  Task 6 without an architecture revision or source change. The accepted
  implementation is recorded by `07cd193` and the composed aggregate/replay
  verification by `9d936da`; the final branch gate remains green at 331
  workspace tests and 228 `solar-sim` Steam-feature tests, with both clippy
  configurations, formatting, and diff checks passing. WP16 remains deferred,
  and its isolated Steam stash was not applied or modified.
- **2026-07-17** — Completed stabilization Task 7 and returned WP7 to
  **✅ done** after re-reading the locked plan and closing every retained
  update-efficiency acceptance item. The command boundary now bypasses Bevy
  mutation tracking while reducers run, compares complete semantic
  before/after state, and marks clock, camera, presentation, View Options,
  navigation, settings, and transient UI resources only when their values
  actually change. Duplicate commands and repeated clamped time edits retain
  exact state flags, while explicit Apply/Restore actions still request
  durable settings writes. Stable input ownership and pointer-capture state
  likewise publish no false changes.

  Desktop and headless execution now share an exact f64 simulation-time
  propagation stamp: paused, UI-only, rate-only, and range-pinned frames reuse
  bit-identical 66-body truth, while each distinct `SetTime`, playback,
  reverse-from-pin, or eased-LIVE time propagates exactly once. Orbit geometry
  uses the complete drifted elements, effective mean motion, and parent GM as
  its exact key. Its documented reuse error is zero kilometres and zero render
  units; fresh-versus-retained tests cover all eight secular planets at the
  1800/2300 limits, catalog epoch, and high-confidence boundary, both fitted
  hyperbolic and two-body-GM paths, and the complete catalog-derived
  smallest-body-to-Sedna zoom domain without changing physics or tolerances.

  Stable emphasis, labels/reticles, body transforms/visibility, orbit lines
  and Gizmo assets, time controls, HUD, search, and breadcrumb values now
  avoid component/material/asset rewrites. Retained render keys preserve
  settings, Layers/rail, Browse, and left-panel entity identity for stable or
  unrelated values; required structural rebuilds retain semantic focus and
  scroll. Default-valued View Options overrides are canonicalized away, and
  camera yaw/zoom no longer scans all body presentation when selection is
  unchanged. Review also found and fixed a hidden-UI edge: an external
  Settings open/close can no longer consume the saved rail target before
  `SHOW UI` restores it.

  The composed real-catalog lifecycle now drives the actual text/modal input
  router, exact single-owner Escape commands, hovered-scroll wheel capture
  versus viewport Dolly, focus-driven scrolling, Jupiter collection → Io
  navigation, cue-less settings recovery, Saturn's real sphere/ring/text/orbit
  aggregate plus Io's architecture-valid reticle, and variable-wall-delta
  LIVE replay with an identical parsed stream and final hash. Independent
  architecture, command/simulation, and retained-UI reviews found no remaining
  Task 7 blocker. Final evidence is green: `cargo test` passes **331 tests**
  (53 `sim-core` · 227 `solar-sim` · 48 `xtask` lib · 2 xtask smoke · 1
  active spot-check); `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and
  `git diff --check` pass; the untouched compatibility path passes 228
  `solar-sim` tests and zero-warning clippy with `--features steam`. Q16
  remains open, WP13 remains `blocked(Q16)`, and no Saturn icon, Steam/WP16
  source, dependency, catalog/generated/truth asset, persisted schema, or
  numerical-tolerance change was made.
- **2026-07-17** — Began the required pre-code review for stabilization Task
  7, reopening WP7 as the coordinating retained-UI/update-efficiency package
  from the green 303-test Task 6 commit. Re-reading ARCHITECTURE invariants
  4, 6–8 and §§7, 8.2–8.4, 9, 10, and 12 plus the locked Task 7 acceptance
  found a shared Bevy change-detection defect rather than isolated repaint
  problems. The desktop command gate mutably dereferences clock, camera,
  View Options, navigation, Browse, settings, and transient UI state for every
  command even when that reducer branch is a no-op. Stable selection and
  presentation-convergence systems repeat the same mistake. An unrelated or
  duplicate UI command can therefore advertise false clock, camera,
  navigation, and `AppSettings` changes, re-propagate all 66 bodies while
  paused, reapply window/runtime settings, rebuild the actionable breadcrumb
  and other surfaces, and trigger unrelated body/UI work.

  The reviewed correction will preserve reducer order while mutating through
  Bevy's change-detection bypass, compare semantic before/after state, and
  explicitly mark only resources that actually changed. Desktop propagation
  and the headless replay runner will share an exact simulation-time reuse
  policy: unchanged time reuses bit-identical `BodyStates`, while `SetTime`,
  LIVE easing, ordinary playback, or changed startup catalog truth still
  propagates. Catalog truth remains the architecture's immutable startup
  resource; no partial runtime hot-reload contract will be invented.
  Retained orbit geometry will continue to use exact complete conic inputs,
  with a documented zero-kilometre temporal-cache error bound and
  fresh-versus-retained coverage across the supported 1800–2300 endpoints,
  catalog epoch, high-confidence boundary, elliptic secular planets, and the
  hyperbolic mean-motion/parent-GM path. No time bucket, screen-space
  approximation, physics tolerance, or f64 truth change is allowed.

  Stable render work will acquire mutable assets/components only when the
  desired value differs: body/ring emphasis materials, label/reticle
  colors and visibility, retained orbit assets/anchors, camera/body
  transforms, and page-independent body presentation. Stable and idempotent
  settings, Layers, Browse, navigation, and View Options transitions will
  retain surface entity identity; actual structural rebuilds must preserve
  the already-reviewed focus and scroll contracts, while single-control
  visual changes use retained-state repaint where that surface permits it.
  Acceptance evidence will prove paused idle/UI-only/rate-only frames perform
  zero propagation, a paused `SetTime` performs exactly one bit-identical
  propagation, stable emphasis/orbits emit no component or asset rewrites,
  unrelated/duplicate commands leave change flags and surface identities
  stable, and one composed real-catalog lifecycle covers text/modal input,
  scrolling, Jupiter/Io navigation, default recovery, Saturn's real
  sphere/ring/text/orbit aggregate plus Io's architecture-valid reticle, and
  variable-input LIVE replay. Q16 remains open and no forbidden Saturn icon
  will be added. Steam/WP16, dependencies, catalogs, generated/truth assets,
  starfield, persisted schemas, and numerical tolerances remain out of scope.
- **2026-07-17** — Completed every architecture-authorized stabilization Task
  6 source change; WP13 is now `blocked(Q16)` only because the locked wording
  names a Saturn icon that Rev C explicitly forbids. Independent aggregate,
  transition, and UI reviews found no source blocker. The Clock set now
  publishes the signed simulation-time advance produced strictly by
  `SimClock::tick`, after commands and before propagation. Orbit emphasis
  therefore follows actual eased LIVE movement, reverse time, pause, and
  1800/2300 range pins without treating an instantaneous `SetTime` edit as
  sustained speed. Crossfades snap only within one `f32::EPSILON` of their
  exact endpoint so the reviewed 0.25-second transition lands bit-exactly
  after fifteen 60 Hz steps; no assertion tolerance was loosened.

  Saturn's ring attachment now carries its owning catalog body index and
  resolves alpha through that identity instead of a hard-coded name.
  Mercury–Saturn sphere materials, owned ring, text labels, and orbits share
  the same monotone body-indexed blend at both +100 and −100 yr/s; Io proves
  the real architecture-valid Icons-layer reticle path and Uranus remains at
  baseline on initial entry. Global/local orbit visibility still overrides
  brightness, while material handles, retained orbit geometry, transforms,
  f64 `BodyStates`, inflated-pick radius, and ray-hit results remain unchanged.
  Every label hide path now clears focus only when that root owned it, so a
  fully faded label cannot remain invisibly keyboard-activatable. Onset
  consumption is explicitly after `OrbitEmphasisSet`: the toast appears in
  the transition frame, never repeats while held, and emits exactly once after
  release/re-entry.

  Eight new regressions raise the workspace suite from 295 to 303 tests (53
  `sim-core` · 199 `solar-sim` · 48 `xtask` lib · 2 xtask smoke · 1 active
  spot-check); the Steam-feature verification passes 200 `solar-sim` tests
  without changing deferred WP16 code. `cargo test`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test -p solar-sim --features steam`,
  `cargo clippy -p solar-sim --all-targets --features steam -- -D warnings`,
  `cargo fmt --all -- --check`, and `git diff --check` all pass. Stable
  label-color rewrite optimization remains documented for Task 7. No
  dependency, catalog, generated-asset, starfield, physics, or tolerance
  change was made.
- **2026-07-17** — Began the required pre-code review for stabilization Task
  6, reopening WP13 as the coordinating high-rate rendering package from the
  green 295-test Task 5 commit. Re-reading ARCHITECTURE §§7, 8.2, 10.3–10.4,
  and 12 plus the locked Task 6 acceptance found that orbit emphasis currently
  derives phase advance from nominal `rate × wall delta`, not the simulation
  clock's actual tick. LIVE easing can therefore move Mercury by radians in
  one rendered frame while leaving its dot fully visible, whereas a clock
  pinned at 1800/2300 can remain faded with bright orbits despite zero
  propagation. The frame-flow fix will publish
  `abs(t_after_tick − t_before_tick)` from `tick_clock`, after commands but
  before propagation, so eased LIVE motion and clamps are truthful while an
  instantaneous `SetTime` edit is not misclassified as sustained aliasing.

  The render aggregate is also only implicitly associated. Every
  `SaturnRing` marker receives hard-coded Saturn alpha even if it is detached
  or mis-parented; ring attachments will instead carry their owning catalog
  body index from spawn and resolve the blend through that identity. Orbit
  updates already run after the emphasis set, but onset-toast consumption does
  not; it will be explicitly ordered so a transition is consumed in the same
  frame. A label whose root becomes `Display::None` at the reviewed near-zero
  cutoff can retain keyboard focus, so hidden label roots will relinquish
  focus rather than remain invisibly activatable. Stable label-color rewrite
  optimization is deliberately recorded for Task 7, where all steady-state
  render work is reviewed together.

  Acceptance evidence will drive the real catalog at ±100 yr/s and 60 Hz
  through intermediate/full fade and smooth release. It will cover
  Mercury–Saturn sphere materials, Saturn's owned ring, text-label alpha/root
  visibility, a real non-primary Icons-layer reticle pending Q16, orbit
  brightness with global/local visibility precedence, Uranus remaining
  initially un-emphasized, one onset/toast per crossing, LIVE snap and
  pinned-edge truth, and unchanged f64 `BodyStates`, transforms, material
  handles, inflated-pick radius, and ray-hit results. No Steam, dependency,
  catalog, generated asset, starfield, physics, or tolerance change is in
  scope.
- **2026-07-17** — Completed stabilization Task 5 and returned WP14 to
  **✅ done** after the independent final review found no blockers. Startup
  `--reset-settings` now consumes the same semantic
  `ApplySettings(default)` settings reducer as the accessible in-product
  `RESTORE DEFAULTS` action, synchronously persists reviewed defaults, and
  only then derives initial clock/layer/runtime state. Fourteen isolated child
  phases use a nonce-specific non-production settings identifier plus
  cross-platform config-root overrides to prove exact persistence before and
  after parsed CLI reset, the actual in-product action/shared reducer path,
  ordinary golden capture, and reset-plus-capture. Golden-only
  resolution/vsync/frame-cap and view-layer overrides now run under an
  explicit transient persistence policy; external convergence, requested
  deferred saves, and window-close sync saves cannot overwrite the production
  file, while an explicit reset remains durable before capture begins.

  The cue-recovery notice now renders above the ordinary HUD/diagnostics but
  below Search, Browse, and Settings. Its generic teardown recognizes focused
  descendants and hands focus to `SHOW UI`, the first live control in the
  active modal, or the semantic Layers rail action, including replay,
  external-layer, and Settings Apply paths. Composed regressions prove exactly
  one accessible notice across stable updates, no command/settings/save
  mutation merely from appearance, one semantic restore command and reviewed
  default convergence on activation, zero cue notices plus exactly one
  tabbable `SHOW UI` action when UI is off, overlay ordering, and live focus
  after every teardown path. Six new regressions raise the workspace suite
  from 289 to 295 tests (53 `sim-core` · 191 `solar-sim` · 48 `xtask` lib ·
  2 xtask smoke · 1 active spot-check); the Steam-feature verification passes
  192 `solar-sim` tests without changing deferred WP16 code.
  `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test -p solar-sim --features steam`,
  `cargo clippy -p solar-sim --all-targets --features steam -- -D warnings`,
  `cargo fmt --all -- --check`, and `git diff --check` all pass. No
  dependency, catalog, generated-asset, physics, or tolerance work changed.
- **2026-07-17** — Expanded the Task 5 pre-code gate after the independent
  combined review found one further persistence defect. Golden capture uses
  the production settings identifier, replaces the live `AppSettings` with
  capture-only resolution/vsync/frame-cap values, and installs view-specific
  cue layers. The normal settings-convergence system then queues a delayed
  save; because a golden run lasts well beyond the 100 ms debounce, an
  ordinary capture can overwrite user settings and a
  `--reset-settings` capture can undo the defaults it just persisted.

  Golden runtime overrides will therefore use an explicit transient
  persistence policy. A requested reset will still cross the shared
  `ApplySettings(default)` reducer and synchronously persist to the production
  identifier before capture overrides are derived, but capture-time
  convergence, requested saves, and close handling will not enqueue disk
  writes. Isolated persistence evidence will prove a capture lifecycle cannot
  change the exact pre-existing file, while reset-plus-capture leaves reviewed
  defaults durable. This remains within Task 5/WP14; no golden image contract,
  Steam work, dependency, generated asset, physics, or tolerance changes are
  in scope.
- **2026-07-17** — Began the required pre-code review for stabilization Task
  5, reopening WP14 as the coordinating settings/recovery package from the
  green 289-test Task 4 commit. Re-reading ARCHITECTURE §§8.2, 8.5, 9.3, and
  12, the Q15 ruling, and the locked Task 5 acceptance found that the existing
  CLI and in-product resets do not yet share one semantic settings reducer:
  startup mutates `AppSettings` directly, while `RESTORE DEFAULTS` queues
  `ApplySettings(default)`, presentation restoration, and close commands. The
  relaunch test invokes the direct helper rather than parsed
  `--reset-settings`, does not exercise the in-product command sequence through
  persistence, and isolates only `HOME`; platforms preferring another config
  root could therefore touch the production settings identifier.

  The transient cue floor also has two implementation defects. Its z-index is
  above both the Search dropdown and the full-screen Browse modal, so the
  notice can obscure or receive pointer input ahead of the active transient
  surface. When replay, Settings Apply, or another layer control restores a
  cue while the notice owns focus, the generic despawn path does not choose a
  successor and leaves `InputFocus` pointing at a dead entity. The reviewed
  implementation will route startup reset through the same
  `ApplySettings(default)` settings reducer before deriving initial runtime
  state, synchronously persist that result, and use the parsed startup option
  at this testable boundary. Isolated multi-process cycles will use a unique
  non-production settings identifier plus platform config-directory
  overrides, and will prove exact persistence before and after both the CLI
  bootstrap and the actual in-product command path.

  The cue notice will move below Search, Browse, and Settings while remaining
  above the ordinary HUD. Every cue-root despawn will resolve focused
  descendants to `SHOW UI` when UI becomes hidden, the active modal when one
  exists, or the semantic Layers rail action otherwise. Composed lifecycle
  regressions will prove one accessible notice across repeated updates, no
  command/settings/save mutation merely from appearance, one semantic restore
  command on activation, exact default convergence, zero cue notices plus one
  tabbable `SHOW UI` control when UI is off, the overlay hierarchy, and live
  focus after external layer and Settings restore paths. No Steam, dependency,
  catalog, generated asset, physics, or tolerance work is in scope.
- **2026-07-17** — Completed stabilization Task 4 and returned WP7 to
  **✅ done** after an independent final source review found no blockers.
  Breadcrumb items now retain semantic root/body/collection destinations
  behind their stable route IDs, and `(depth, target_id)` is validated
  atomically against the current stack before either the camera or application
  state can change. Desktop and headless reducers converge selected-body,
  panel, and navigation state after each ordered command; unsupported
  collection pages reject without mutation; Jupiter Collection/Info/View,
  Io, current/ancestor/root breadcrumb routes, and same-frame versus
  split-frame sequences now produce the documented canonical states.
  Breadcrumb rebuilding is explicitly ordered after navigation convergence
  and restores focus to a live semantic successor.

  Search input and its dropdown now share TextEdit ownership, so focused
  results suppress all background gameplay input and Escape uses the shared
  cancellation path. Keyboard and pointer selection each queue exactly one
  Travel command, commit the displayed value, restore live Search focus, and
  suppress exact-match popup reopening until a fresh edit. Browse preserves
  Travel-then-close ordering and returns focus to its live Menu invoker, with
  stale-target clearing as the fallback. Replay command text remains
  compatible; semantic navigation identity intentionally changes the pinned
  portable state hash to `1535747298578131566`. Fifteen new regressions raise
  the workspace suite from 274 to 289 tests (53 `sim-core` · 185 `solar-sim`
  · 48 `xtask` lib · 2 xtask smoke · 1 active spot-check). Evidence:
  `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, and `git diff --check` all pass. No Steam,
  dependency, catalog, generated-asset, physics, or tolerance work changed.
- **2026-07-17** — Began the required pre-code review for stabilization Task
  4, reopening WP7 as the coordinating breadcrumb/navigation package from the
  green 274-test Task 3 commit. Re-reading ARCHITECTURE §§8.2, 8.4, 9.1, and
  12 plus the locked Task 4 acceptance exposed coupled state-machine defects.
  Collection crumbs use synthetic IDs such as `jupiter_moons`, but both
  reducers require a catalog body and therefore render an actionable no-op.
  Breadcrumb commands validate depth and target independently, so stale,
  mismatched, or out-of-range pairs can partially mutate camera, page, or
  stack. `SetLeftPanelTab(Collection)` also accepts bodies with no moon
  collection, while command batches synchronize selected-body navigation only
  after every command, causing a later same-frame tab command after Travel or
  breadcrumb navigation to be overwritten. Selection-derived navigation and
  breadcrumb rebuilding have no explicit same-frame ordering, and a focused
  ancestor button is despawned without a semantic successor. Search-result
  focus is currently misclassified as Gameplay, allowing background hotkeys
  and making Escape ineffective; Search selection clears focus, while Browse
  closes to Search rather than its Menu invoker and can immediately reopen a
  retained-query dropdown.

  The reviewed implementation will give each `NavigationItem` a semantic
  destination (root/body/collection) while retaining stable route IDs and
  replay text compatibility; validate `(depth, target_id)` atomically against
  the current stack; resolve camera body plus panel page from that one target;
  reject unsupported collection tabs without mutation; and converge selected
  body/navigation after every accepted command so recorded order is honored.
  Breadcrumb rendering will run after selection convergence and restore focus
  by semantic route. Search input plus its dropdown will share TextEdit
  ownership; a common commit/cancel path will return focus to Search while
  suppressing exact-match popup reopening until a fresh edit, and Browse will
  return focus to Menu with a live-entity fallback. Acceptance evidence will
  cover the exact Jupiter/Io paths, current and ancestor breadcrumb actions,
  malformed/stale rejection, same-frame versus split-frame command order,
  replay/headless parity, pointer and keyboard Search/Browse selection,
  gameplay-hotkey suppression, Escape cancellation, focus survival, and a
  second-update no-reopen assertion. No Steam, dependency, catalog, generated
  asset, physics, or tolerance work is in scope.
- **2026-07-17** — Completed stabilization Task 3 and returned WP7 to
  **✅ done** after two independent source gates and a final no-blocker
  acceptance review. Non-modal surfaces now use ordered tab groups
  (top/search/left/time/rail/layers/recovery), unique semantic indices, and a
  central `ScrollIntoView` observer for registered scroll surfaces. Settings,
  the left panel, all three Browse columns, the search dropdown, right rail,
  layers panel, and breadcrumb retain or deliberately reset scroll/focus
  across rebuilds; semantic left-panel transitions cannot be overwritten by
  outgoing live scroll, Browse close/reopen cannot recover stale actions, and
  UI-off plus cue-recovery paths always hand focus to a live modal or rail
  control. The top bar uses a flex-integrated scrollable breadcrumb, the time
  bar uses two compact rows, long Browse labels wrap inside bounded controls,
  the left-panel tabs live inside its scroll region, Settings has an
  auto-height wrapping footer and the supported 2.0 scale step, and all
  transient rails/dropdowns are constrained between fixed HUD bars. The
  headless matrix fixture now runs Bevy's real Inter text measurement,
  editable-content sizing, Taffy layout, glyph layout, transforms, and
  clipping. Twenty-nine new regressions raise the workspace suite from 245 to
  274 tests (53 `sim-core` · 170 `solar-sim` · 48 `xtask` lib · 2 xtask smoke
  · 1 active spot-check), including all seven UI surfaces at 800×600 and
  960×600 with scales 0.75, 1.0, 1.5, and 2.0, real Tab/Shift-Tab search
  activation, line/pixel scroll clamping, wheel-versus-Dolly ownership, and
  command/replay-driven focus recovery. `cargo test`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, and `git diff --check` are green. No Steam,
  catalog, generated asset, dependency, physics, or tolerance scope changed.
- **2026-07-17** — A second independent Task 3 source gate found three more
  acceptance blockers before closure. The headless layout fixture currently
  replaces Bevy's `PostUpdate` UI pipeline with layout/transforms only, so it
  omits text/editable-content measurement and can falsely pass controls whose
  real text would overlap or clip. The newly tabbable search-results group is
  also incompatible with the existing edit lifecycle: moving focus from the
  Search input to a result ends editing and despawns that focused dropdown in
  the same frame. Finally, activating the cue-recovery action despawns its
  focused entity without choosing a semantic successor. The integrated
  correction must restore a representative Bevy text-measurement/layout
  pipeline in the fixture; keep the search session alive while focus is
  within its input-or-dropdown ownership surface and prove keyboard result
  activation; and restore cue-recovery focus to a stable rail action after
  its command removes the notice. These regressions join the four already
  documented gate failures and all must pass the real eight-case matrix
  before Task 3 can be completed.
- **2026-07-17** — Task 3 remains in progress after the independent
  acceptance gate found four coupled continuity defects in the first
  implementation. Intentional left-panel resets from tab, breadcrumb, and
  selected-body transitions are currently overwritten by the outgoing live
  `ScrollPosition`; Browse close clears its semantic focus target in the
  reducer but the closing rebuild captures that stale target again; a
  command-driven UI restore can leave focus on the now-hidden modal `SHOW UI`
  affordance; and the three-column Browse layout still gives long no-wrap
  labels no horizontal or wrapping path at 800×600 with UI scale 2.0. The
  reviewed correction keeps these concerns integrated: mark semantic
  left-panel scroll resets so rebuild snapshotting cannot override them;
  capture Browse action focus only while the menu remains open and prove a
  close/reopen starts at `CLOSE`; treat `SHOW UI` as the canonical modal
  focus while UI is off and move command-restored focus to the rebuilt rail
  (or the active higher-priority modal); and make Browse titles/actions
  width-constrained and wrapping, with the eight-case Bevy layout regression
  checking text bounds as well as button reachability. Dedicated regressions
  for each failure are required before the full Task 3 verification suite and
  WP7 completion evidence are rerun.
- **2026-07-17** — Began the required pre-code review for stabilization Task
  3, reopening WP7 as the coordinating UI work package after Task 2 returned
  WP5 to done. Re-reading ARCHITECTURE §§8.2, 8.4, 8.5, 9, and 12 plus the
  locked Task 3 matrix exposed coupled continuity and reachability defects:
  non-modal HUD controls have `TabIndex` components but no `TabGroup`
  ancestors; Settings, the left panel, and time controls reuse equal indices
  instead of declaring semantic order; focus restoration can target despawned
  entities or jump to an unrelated modal action; focused offscreen controls do
  not scroll into view; and the Settings UI-scale action omits the supported
  2.0 value. At 800×600 or 960×600 with scales 1.5–2.0, fixed top/time bars,
  the absolute breadcrumb, the unconstrained right rail/layers panel, the
  search dropdown, the left-panel chrome, and the fixed-height Settings footer
  clip or overlap required controls. The integrated implementation will add
  ordered surface tab groups and unique semantic indices, central
  focus-to-scroll behavior at the existing `UiScrollSurface` boundary,
  retained/constrained rail and layers scrolling, semantic focus snapshots
  with explicit fallbacks, Browse expansion continuity, a flex-integrated
  breadcrumb, a compact two-row time bar, a bounded scrollable search
  dropdown, a narrower left panel with tabs inside its scroll region, an
  auto-height Settings footer, and the missing 2.0 scale step. Acceptance
  evidence will use actual Bevy layout across all eight required
  resolution/scale pairs, first/last-control scroll reachability,
  Tab/Shift-Tab order, real wheel-versus-Dolly routing, and rebuild focus/scroll
  regressions before WP7 is returned to done. Full-tree repaint optimization
  remains Task 7; no Steam, catalog, generated asset, dependency, physics, or
  tolerance work is in scope.
- **2026-07-17** — Completed stabilization Task 2 and returned WP5 to
  **✅ done** after an independent architecture review found no remaining
  blocker. `InteractionState` is the sole frame-latched interaction-context
  resource; it is captured after Bevy input collection and before both
  focused-keyboard and picking dispatch, while scroll-hover capture remains
  non-context pointer-routing data. Text editing, Browse, and Settings now
  suppress gameplay keys, right-drag, wheel dolly, label activation, and
  viewport picking for the whole owned frame. Browse and Settings are
  command-reducer-exclusive, modal focus is seeded and reconciled inside the
  active tab group, and focused buttons own Space without also triggering the
  global playback binding. Date/time Escape restores the pre-edit display and
  emits no command; Enter emits exactly one `SetTime`. Twelve new
  ownership/focus/cancellation regressions raise the workspace suite from 233
  to 245 tests (53 `sim-core` · 141 `solar-sim` · 48 `xtask` lib · 2 xtask
  smoke · 1 active spot-check). `cargo test`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, and `git diff --check` are green. No Steam,
  catalog, generated asset, dependency, or numerical-tolerance change is
  included.
- **2026-07-17** — Began the required pre-code review for stabilization Task
  2 (WP5 input ownership) after re-reading ARCHITECTURE invariants 4 and 7
  and the locked Task 2 acceptance. The audit found five coupled defects:
  date/time Escape has no cancel path and valid edits commit on every focus
  loss; Browse and Settings create modal tab groups without placing focus
  inside them; focused UI buttons and the global Space binding can both act
  on one keypress; Browse and Settings can coexist despite the single-context
  model; and label/viewport guards depend on an `InteractionState` snapshot
  that is stale during the next PreUpdate after a modal command. The scoped
  design is therefore integrated: retain an explicit time-edit cancellation
  snapshot, deterministically seed modal focus, suppress raw activation keys
  owned by the focused widget, make ordered modal-open commands mutually
  exclusive with a defensive TextEdit → Browse → Settings resolver, and
  derive label/pick blocking from canonical focus/Browse/Settings state at
  dispatch time. Acceptance evidence will include real focused-input,
  Tab/Shift-Tab containment, same-key single-command, Escape-order, pointer,
  label, and viewport regressions before Task 2 is marked complete. No Steam,
  catalog, generated asset, dependency, or unrelated rendering work is in
  scope.
- **2026-07-17** — Completed stabilization Task 1 (WP5 command boundary and
  replay schema) before beginning Task 2. `ReplayStream` now retains an
  explicit v1/v2 version, v1 round-trips without upgrading, and v2 always
  requires its exact ordered finite frame-input set instead of silently
  falling back to synthetic v1 timing. Desktop and headless execution now
  share the presentation, View Options, left-panel/navigation, Browse,
  settings, and persistence-convergence reducers; application commands are no
  longer discarded while catalog/camera resources are unavailable. Settings
  open/closed state is canonical rather than a desktop-only transient request.
  The combined replay hash now covers exact wall time, every layer,
  fullscreen, Settings/Browse modal state, full navigation identity, View
  Options, and normalized settings while continuing to exclude render-only
  state. Frame recordings now stamp every same-frame command with the
  frame-start time, so a `SetTime` followed by another command replays
  correctly, and invalid breadcrumb targets cannot partially mutate UI state.
  The portable 600-frame hash intentionally changed from
  `11341847874983838712` to `1553394718950124988`; no numerical tolerance or
  physics assertion changed. Ten new transition/rejection/parity tests raise
  the workspace baseline from 223 to 233 tests (53 `sim-core` · 129
  `solar-sim` · 48 `xtask` lib · 2 xtask smoke · 1 active spot-check).
  `cargo test`, `cargo clippy -p solar-sim --all-targets -- -D warnings`,
  `cargo fmt --all`, and `git diff --check` are green. WP5 remains
  `in-progress` only for the documented Task 2 input-ownership acceptance.
- **2026-07-17** — Reopened WP5 for the human-approved UI/gameplay remediation
  and deferred WP16 while the Steam/overlay investigation remains explicitly
  on hold. The pre-code audit found that the replay-v2 stream does not retain
  its parsed version, the combined replay hash omits layer, presentation, and
  wall-time state, and desktop/headless application reducers have diverged.
  Task 1 will repair those defects against ARCHITECTURE invariants 4 and 7 and
  the reviewed acceptance matrix in
  `docs/ui-gameplay-stabilization-2026-07-16.md`; no Steam runtime, overlay,
  packaging, App ID, dependency, catalog, or generated asset work is in scope.
  The paused six-file WP16/MSAA delta is preserved without loss in stash
  `hold-wp16-steam-msaa-2026-07-16`, and branch
  `codex/ui-gameplay-remediation` starts from committed stabilization baseline
  `0e49870`. The required pre-change `cargo test` is green at the committed
  223-test baseline (53 `sim-core` · 119 `solar-sim` · 48 `xtask` lib ·
  2 xtask smoke · 1 active spot-check) before source changes.
- **2026-07-16** — Completed the human-directed UI/gameplay stabilization
  cycle and closed Q15 with both approved recovery measures. The reviewed
  implementation brief and task-by-task acceptance matrix live in
  `docs/ui-gameplay-stabilization-2026-07-16.md`. All application-visible View
  Options, Browse, settings commit/reset, left-panel, and breadcrumb
  transitions now cross `SimCommandQueue`; replay-v2 records wall delta and
  wall-clock TDB inputs for deterministic LIVE snaps while replay-v1 remains
  parseable. Text, Browse, and Settings contexts suppress background hotkeys,
  orbit, dolly, label activation, and viewport picking; long Settings,
  left-panel, and Browse surfaces retain/clamp scroll state and keyboard focus
  is restored across rebuilds. The breadcrumb is actionable, cue-less persisted
  views expose one transient restore notice, `--reset-settings` performs an
  isolated write/relaunch/reset/relaunch persistence cross-check, and Saturn's
  rings now share its high-rate fade. Paused clocks skip unchanged propagation
  and secular orbit resampling, and steady emphasis no longer rewrites
  materials. The portable replay hash intentionally changed from
  `11614332433107791956` to `11341847874983838712` because the combined hash
  now includes View Options, settings, navigation, and modal state; no numeric
  tolerance or assertion was weakened. `cargo test` passes the increased
  223-test baseline (53 `sim-core` · 119 `solar-sim` · 48 `xtask` lib · 2
  xtask smoke · 1 active spot-check), `cargo test -p solar-sim --features
  steam` passes 120 tests, both required clippy invocations pass with
  `-D warnings`, and formatting/diff checks are clean. No dependency, catalog,
  truth fixture, `ARCHITECTURE.md`, `AGENTS.md`, WP status, or acceptance text
  changed.
- **2026-07-16** — Finalized the Settings recovery follow-up after the human
  reported that the real macOS release build now passes pointer adjustment,
  scrolling, `REVERT`, `APPLY`, `CLOSE`, and Escape. Hosted
  [run 29488349896](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29488349896)
  is green for commit `60a19a6718edbc3b239606325f1b663c723d5a12`:
  `invariants` 30s, `lint` 47s, `test-linux` 1m54s, `platform (macos-14)`
  3m03s, and `platform (windows-latest)` 8m17s. `README.md` now documents the
  operational Settings controls and current 212-test baseline;
  `docs/wp16-steam-overlay-spike.md` records the validated non-Steam usability
  baseline so the later overlay retest is not confounded with this resolved
  defect. Q15 is narrowed but remains open for the separate persisted
  visual-cue recovery choice. No WP status or acceptance criterion changed.
- **2026-07-16** — Follow-up to the Settings lock-in repair: the human
  confirmed that Escape dismissed the modal but every pointer-operated
  setting remained inert. A focused contract assertion added to
  `settings_screen_renders_every_control_with_accessibility_labels` reproduced
  the defect exactly: `cargo test -p solar-sim
  settings::tests::settings_screen_renders_every_control_with_accessibility_labels
  -- --exact` found 0 of 38 setting actions carrying Bevy 0.19's
  `ui_widgets::Button`. The shared `ui_kit` chip, checkbox, slider, and tab
  segment scenes had resolved the unqualified `Button` name to the legacy
  visual `bevy_ui::widget::Button`, which does not emit the `Activate` event
  consumed by the app's observers. All four scenes now use
  `bevy::ui_widgets::Button` explicitly. The same focused command now passes
  with all 38 controls pinned to the action-emitting component. `cargo test`
  passes the 212-test workspace baseline, `cargo test -p solar-sim --features
  steam` passes 109 tests, `cargo clippy --workspace --all-targets -- -D
  warnings` passes, and `cargo fmt --all -- --check` plus `git diff --check`
  are clean. No dependency, settings schema, acceptance text, or WP status
  changed.
- **2026-07-16** — Fixed the WP14 Settings-screen lock-in reported during the
  Mac-first WP16 bring-up, without resuming overlay work. The screen is now a
  full-window modal tab group with a blocking scrim and a real scroll position;
  pointer scrolling is clamped to the content range. `CLOSE`, `APPLY`, and the
  Escape key all route through the deterministic `SimCommand` boundary, closing
  clears input focus, and gameplay orbit/dolly/key intents are suppressed while
  the modal is open. `APPLY` also dismisses the modal, so applying a hidden-UI
  layer state cannot strand the user behind an invisible settings screen.
  Regression tests cover the spawned modal's close path, close-request focus
  cleanup, line/pixel scroll clamping, modal input suppression, presentation
  reduction, and replay serialization. Evidence: the pre-change `cargo test`
  passed the 208-test baseline; post-change `cargo test` passes 212 tests (53
  `sim-core`, 108 `solar-sim`, 48 `xtask` lib, two xtask smoke, one active
  spot-check), and `cargo test -p solar-sim --features steam` passes 109 tests.
  `cargo fmt --all -- --check`, `git diff --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, and `cargo clippy -p solar-sim --all-targets
  --features steam -- -D warnings` pass. The exact local `cargo run -p solar-sim
  --release -- --smoke 120 --expect-backend metal --reject-software-adapter
  --assert-nonblack` launch completes at 85.0 fps on the Apple M2 Pro and reports
  a non-black 5120×2880 readback. Q15 remains open for the separate persisted
  visual-cue recovery decision; no WP14 or WP16 status/acceptance text changed.
- **2026-07-16** — The first real-client M2 Pro overlay attempt found four
  WP16 integration defects before it could count as spike evidence. The supplied
  terminal log shows Metal `AdapterInfo` before `[S_API] SteamAPI_Init()`,
  `steam: initialized app_id=480 overlay_available=false`, and unresponsive
  Shift-Tab. `codesign -d --entitlements - target/release/solar-sim` showed no
  entitlements. Platform initialization now precedes Bevy `DefaultPlugins`;
  the adapter pumps Steam callbacks every frame and refreshes `PlatformStatus`;
  `prepare-steam-dev` ad-hoc signs the binary with Valve's two required macOS
  overlay entitlements. The focused platform tests pass 3/3 with the new
  delayed-overlay transition test, the focused xtask tests pass 4/4 with the
  entitlement contract, and feature-enabled clippy passes with warnings
  denied. Full `cargo test` passes the new 208-test default baseline (53
  `sim-core`, 104 `solar-sim`, 48 `xtask` lib, two xtask smoke, one active
  spot-check), and the feature-enabled app suite passes 105 tests. The corrected
  local `codesign` inspection reports both entitlements, and the Steam-enabled
  60-frame Metal smoke prints `[S_API]` before adapter reporting, passes the
  backend and real-GPU checks at 179.5 fps, and confirms a 1920×1200 non-black
  readback. A separate 15-second launch still reported no overlay transition,
  while `vmmap` showed Steam API/client images but no overlay renderer. The
  remaining global/per-Spacewar client setting and launch-injection check
  therefore stays with the human. The macOS overlay result remains pending a
  corrected Shift-Tab rerun; no WP16 acceptance box changed. Q15 records the
  separate persisted-settings recovery ambiguity without changing WP14.
- **2026-07-16** — WP16's default-build dependency isolation is accepted at
  commit `ad9be42b12347acee4d2d4f17776199f0f6a9dd1`. The exact local
  `cargo tree -p solar-sim --edges normal --no-default-features` check contains
  no Steamworks crate, while `cargo tree -p solar-sim --edges normal --features
  steam` resolves `steamworks v0.13.1` and `steamworks-sys v0.13.0`. Hosted PR
  [run 29473492755](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29473492755)
  passed in 54m26s: `lint` 5m09s, `test-linux` 20m44s, `invariants` 33s,
  `platform (macos-14)` 3m33s, and `platform (windows-latest)` 54m20s. The
  opt-in Steam adapter compiled and linked on both hosted platform legs (23s
  on `macos-14`, 17m44s on `windows-latest`); the macOS Metal smoke passed in
  9s and the expected Windows `Microsoft Basic Render Driver` / `Cpu` / `Dx12`
  soft smoke passed in 2m05s. This checks only WP16's first acceptance item;
  the overlay, signing/SteamPipe, and bundle-size items remain open.
- **2026-07-16** — WP16's Mac-first Steam adapter and interim-identity
  guardrails landed after the human closed Q14 and narrowed Q13. Optional
  `steamworks = "0.13.1"` is reachable only through the `steam` feature;
  `SteamPlugin` initializes the single provenance-commented App ID 480,
  exposes only `PlatformServices`, drops the client on Bevy exit, and falls
  back to the overlay-unavailable no-op adapter when Steam is absent. The
  `xtask prepare-steam-dev` command generates the ignored `steam_appid.txt`
  beside the built app from that same Rust source. The package and depot
  `steam-release-preflight` modes both exit 1 with the required
  interim-Spacewar refusal. `docs/wp16-steam-overlay-spike.md` records the
  exact M2 Pro commands and leaves the real-client result pending the human
  run; Windows and SteamPipe evidence remain deferred under open Q13. `cargo
  test` passes the new 206-test default baseline (53 `sim-core`, 103
  `solar-sim`, 47 `xtask` lib, two xtask smoke, one active spot-check). The
  feature test command passes all 104 `solar-sim` tests; both default-workspace
  and feature-enabled clippy commands pass with warnings denied. The local
  feature-enabled 60-frame smoke command exits 0 on `Apple M2 Pro` / Metal
  after Steam initialization reports no running client, proving the app still
  runs with the overlay unavailable. No WP16 acceptance box is changed in this
  entry; hosted Mac/Windows feature-link evidence is pending.
- **2026-07-14** — WP16 moved to **in-progress** with the dependency-free
  platform boundary completed before the hardware overlay spike. The app now
  installs `PlatformServicesPlugin` with an overlay-unavailable no-op default;
  application lifecycle code sees only the `PlatformServices` trait and shuts
  an injected implementation down once on `AppExit`. The new mock test
  `platform::tests::app_lifecycle_uses_the_mock_and_does_not_require_an_overlay`
  passes both alone and in `cargo test`; the full command passes all 201 tests
  (53 `sim-core`, 102 `solar-sim`, 43 `xtask` lib, two xtask smoke, one active
  spot-check). `cargo clippy -p solar-sim --all-targets -- -D warnings` and the
  exact default-tree guard command from `.github/workflows/ci.yml` pass. Q14
  records the required dependency approval, App ID, and injection decision; no
  dependency was added and no WP16 acceptance box was checked.
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
