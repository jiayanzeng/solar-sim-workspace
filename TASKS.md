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
| 0 | Workspace, Bevy 0.19 pin, CI, window+camera+diagnostics, core-purity rule | **in-progress** (workspace + crates exist; Bevy app, toolchain pin, CI absent; human guide: `docs/wp0-dev-setup-macos.md`) |
| 1 | `sim-core::time` — full ladder, start epoch, LIVE, range | **✅ done** |
| 2 | `sim-core::kepler` — elliptic + hyperbolic, guards | **✅ done** |
| 3 | `xtask gen-catalog` + committed 66-body `catalog.ron` + validation | **in-progress** (pipeline ✅; online capture blocked on Q5; curated review brief ready) |
| 4 | Propagation + floating origin: 66 colored spheres at 2026 positions | todo (unblocked by WP0) |
| 5 | Camera rig, input-intent layer, key map, travel tween, replay determinism | todo |
| 6 | Orbit lines (adaptive; hyperbolic arc), colors, fades | todo |
| 7 | `ui_kit`: theme, fonts, BSN widgets, top bar + breadcrumb | todo |
| 8 | Time bar: detented log slider, editable date/clock, LIVE chip | todo (binds to WP1 API) |
| 9 | Labels/reticles, tiered declutter, contextual moon visibility, picking | todo |
| 10 | Left panel: Info tab, collection pages, View Options | todo |
| 11 | Layers quick panel, right rail, Icons layer, UI-off mode | todo |
| 12 | Search (alias-aware) + Menu browse with live counts | todo |
| 13 | Orbit-emphasis high-rate mode; BSC starfield; Sun bloom | todo |
| 14 | Settings screen + render-recovery policies | todo |
| 15 | Texture pass (2K KTX2) + visual polish + golden screenshots | todo |
| 16 | Steam: Steamworks init, overlay spike, packaging/signing/depots | todo |
| 17 | QA: replay suite, perf gates, demo script, licensing audit | todo |
| 18 | *Optional:* Compare Size mode | deferred |

**Test baseline: 71 passing** (51 `sim-core` · 17 `xtask` lib · 2 smoke ·
1 spot-check harness, dormant). Any change that lowers this number without
an accompanying change-log justification is a regression. The number may
only go up (72 expected after the `UNIX_EPOCH_JD` warm-up patch).

---

## Done (evidence)

### WP1 — `sim-core::time` ✅
`crates/sim-core/src/time.rs` (21 tests). RateIndex ±1..±12 (no zero), 24
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
`crates/sim-core/src/catalog.rs` (16 tests), `xtask/*` (17 lib + 2 smoke).
Schema v1 with collect-all validation + lints; 66-body curated manifest
with count/order/GM tests; Horizons + SBDB parsers; fitted secular/mean-
motion normalization; `--dry-run` / `--fixtures --allow-partial` /
feature-gated `--online`; provenance-headed emission; offline smoke
produces `assets/catalog.sample.ron` (6 bodies) that reloads through the
app loader. Spec: `docs/wp3-gen-catalog-spec.md`.

---

## WP3 — remaining to close (acceptance not yet met)

- [ ] **Online capture run** (needs JPL network access; run
  `cargo run -p xtask --features online -- gen-catalog --online --out
  assets/catalog.ron`). Commit the emitted file *and* the captured API
  responses for reproducibility. **Currently `blocked(Q5)`** — the
  2026-07-12 attempt failed at Jupiter (`no $$SOE in Horizons result`);
  see Open question Q5 for the diagnosis and proposed route fix. Also
  land the diagnostics hardening from Q5 (dump raw response on parse
  failure; `--capture DIR` writing every raw response) before the real
  run, so the capture doubles as the committed reproducibility record.
- [ ] **TNO moon resolution**: Horizons lookup-API resolution for
  Dysnomia / Hiʻiaka / Namaka COMMANDs and Eris/Haumea center designators
  (`xtask/src/lib.rs` lookup route; spec §8 item 1). Until then: fixtures.
- [ ] **Curated review pass**: clear every `TODO(review)` in
  `xtask/src/manifest.rs` (all radii; GMs for Pluto / Eris / Haumea;
  3I/ATLAS nucleus radius). Research brief with citations and
  recommendations: `docs/open-questions-brief-2026-07-12.md` (Q2, Q3).
  Human sign-off required; agents prepare the diff + citations only.
  Note the Pluto-GM semantics question raised there (Pluto-only 869.6 vs
  Pluto+Charon ≈ 975.5 for correct Charon period under μ=parent-GM
  propagation) — that is part of Q2's human decision.
- [ ] **Spot-check activation**: capture Horizons VECTORS for the 10-body
  set (ARCHITECTURE §5.6) at JD 2461042.0 and 1986-02-09 into
  `xtask/fixtures/spotcheck/vectors.json`; document per-category
  tolerances in `docs/wp3-gen-catalog-spec.md`; `cargo test` must show the
  gate passing (not skipping).

## WP0 — remaining to close

Human walkthrough for every step below: `docs/wp0-dev-setup-macos.md`.

- [ ] `rust-toolchain.toml` pinning **1.95.0** — Bevy 0.19.0's declared
  MSRV per crates.io (Q1, answered pending close). Keep `sim-core`'s own
  MSRV claim conservative (`rust-version = "1.75"` in its Cargo.toml).
- [ ] `crates/solar-sim` skeleton: Bevy 0.19.x pinned in `Cargo.toml`
  (`bevy = "0.19"` + committed `Cargo.lock` = exact pin), window opens,
  orbit camera stub, dev-only `DiagnosticsOverlay`, `--smoke` flag
  (render N frames, exit 0) for CI launch checks.
- [ ] CI (GitHub Actions): fmt, clippy (deny warnings), nextest,
  macOS + Windows build jobs, **core-purity rule** (fail if `sim-core`'s
  dependency tree contains any `bevy*` crate), offline rule (no `online`
  feature — and therefore no `ureq` — in default/CI builds).
- [ ] Acceptance: app opens on macOS; Windows job compiles and links in
  CI; CI green. Full "window opens on real Windows hardware"
  verification is tracked as a deferred checkbox (no Windows machine on
  hand): — [ ] Windows launch verified (hardware/VM), due before WP16.

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
- [ ] With the real `catalog.ron`, heliocentric longitudes of the 8
  planets at the 2026 epoch match Horizons to eyeball accuracy (or, once
  the spot-check data exists, the WP4-side positions match `sim-core`'s
  spot-checked output bit-for-bit).
- [ ] No visible jitter at closest zoom focused on Mercury; none focused
  on Sedna (the two precision extremes).
- [ ] All 66 bodies render; frame flow order is input → commands → clock
  → propagation → origin → render (verified by system ordering, not luck).
- [ ] Perf budget holds with all 66 bodies.

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
- [ ] Zero direct sim-state mutation outside the command consumer
  (enforced by module visibility or a grep-able convention documented in
  the code).
- [ ] Travel tween to a moving target (e.g. Io) lands and follows without
  a snap.
- [ ] Zoom clamps hold at both ends.
- [ ] Replay determinism test runs in CI on macOS and Windows and produces
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
- [ ] Ellipses close with no visible seam gap; Nereid (e=0.75) shows
  visibly denser sampling near perihelion.
- [ ] 3I/ATLAS renders an open arc spanning ±25 yr around perihelion and
  never a closed loop.
- [ ] No orbit-line jitter or z-fighting flicker at full-system zoom while
  focused on an inner body.
- [ ] Perf budget holds with all orbit lines on.

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
- [ ] Widget gallery shows every widget in default/hover/active/disabled
  states with the theme applied.
- [ ] Every widget carries an AccessKit label (verified via the gallery).
- [ ] Breadcrumb reflects a scripted navigation-stack push/pop sequence.
- [ ] Font license file vendored beside the font with source noted.

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
- [ ] Dragging across every detent and releasing reproduces the exact
  RateIndex ladder (round-trip through the slider mapping).
- [ ] Typing an invalid date/time reverts and does not move the clock.
- [ ] LIVE chip state matches `is_live` in all four regimes (paused,
  wrong rate, snapping, live).
- [ ] Each toast appears exactly once per transition (scrub into the 1800
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
- [ ] Full-system default view: zero overlapping labels; all 8 planets
  labeled.
- [ ] Focused on Jupiter: its major moons labeled; Saturn's moons not
  (until Saturn is focused/near).
- [ ] Clicking a label and clicking a sphere both select and travel;
  selection always keeps its label.
- [ ] Declutter is stable frame-to-frame (no label flicker while the
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
- [ ] Iterating all 66 bodies programmatically populates the Info tab
  without panic, missing field, or empty period for elliptic bodies.
- [ ] "Moons of X (n)" counts equal the catalog's actual children counts
  for every parent with moons.
- [ ] Size exaggeration changes rendered radius only: picking radius and
  propagation are unaffected (test by picking at ×50).
- [ ] Description shows the curated blurb; empty descriptions surface the
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
- [ ] Every layer toggle takes effect within one frame and is reflected
  in the panel state.
- [ ] UI-off leaves exactly one restore affordance; restoring returns the
  previous layout and layer states.
- [ ] Zoom buttons and scroll wheel produce identical command traffic.

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
- [ ] "3I/ATLAS" and "C/2025 N1" both resolve uniquely to the same body;
  "hale" surfaces Hale–Bopp in the dropdown.
- [ ] For every body, typing its exact name puts it at rank 1 (property
  test over all 66 × {name, designation, aliases}).
- [ ] Menu counts equal catalog category counts (1/8/9/8/32/8) at load.
- [ ] Keyboard-only flow works: focus search, type, Enter travels.

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
- [ ] At +100 yr/s the inner system reads as glowing orbit paths (no
  strobing planet dots) while Sedna and long-period comets still crawl as
  dots.
- [ ] Emphasis engages/disengages per body at rates predicted from its
  period (spot-check Mercury vs Neptune) with no flicker at the boundary.
- [ ] Onset toast fires exactly once per onset.
- [ ] Starfield tilt is correct (ecliptic pole star-field matches
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
- [ ] Every listed setting survives full quit + relaunch.
- [ ] `StartMode::FixedEpoch` boots on the configured epoch;
  `StartMode::Live` boots live — both verified.
- [ ] Simulated device loss recovers to a rendering app without restart.
- [ ] Units toggle updates every visible distance in one frame.

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
- [ ] `catalog.lint()` reports zero untextured star/planet lints.
- [ ] Every shipped texture has license + source metadata; the CI check
  fails on a metadata-less asset (prove by adding one in a scratch branch).
- [ ] Untextured bodies still render with catalog colors (texturing stays
  polish, not a dependency).
- [ ] Goldens are stable across two consecutive CI runs on the same
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

### WP18 — Compare Size mode (deferred)

Optional post-beta. No brief until un-deferred by the human.

---

## Next up (dependency order)

1. **WP0** close-out (human-driven; guide in `docs/wp0-dev-setup-macos.md`),
   in parallel with the Q5 decision that unblocks WP3's online capture.
2. **WP3** close-out: Q5 fix → online capture (+ raw-response commit) →
   spot-check activation → curated review sign-off (brief ready).
3. **WP4 → WP5 → WP6**, then **WP7/WP8** (ui_kit, then the time bar
   binding WP1's API), then WP9–WP15 per briefs, WP16–17 release
   engineering.

## Open questions (humans close these)

| # | Question | Raised | Status |
|---|---|---|---|
| Q1 | Confirm Bevy 0.19.x minimum Rust toolchain and record in WP0 pin | 2026-07-12 | **answered, pending close** — crates.io reports `rust_version = 1.95.0` for bevy 0.19.0 (and all 0.19 RCs). Action: land `rust-toolchain.toml` with `channel = "1.95.0"`, then close. Evidence in `docs/open-questions-brief-2026-07-12.md` §Q1. |
| Q2 | TNO GM values (Pluto 869.6 / Eris 1108 / Haumea 267 km³/s²) — accept or replace with cited values during curated review? Includes the Pluto-GM semantics decision (Pluto-only vs Pluto+Charon ≈ 975.5 for correct Charon period under μ=parent-GM). | 2026-07-12 | open — research brief with citations + recommendation ready (`docs/open-questions-brief-2026-07-12.md` §Q2) |
| Q3 | 3I/ATLAS nucleus radius: literature is uncertain; which value + citation ships? | 2026-07-12 | open — brief recommends adopting R = 0.5 km with the HST-constrained range cited (`docs/open-questions-brief-2026-07-12.md` §Q3) |
| Q4 | Constellation-figure line set licensing (fast-follow; Yale BSC-derived in-house vs licensed) | 2026-07-12 | open — options + recommendation in brief §Q4 (recommend in-house over public-domain BSC) |
| Q5 | **Horizons planet routes: switch giant planets from planet centers (599/699/799/899) to system barycenters (5/6/7/8)?** The 2026-07-12 online run failed at Jupiter (`no $$SOE`). Planet-center ephemerides are defined by satellite solutions with limited time spans, while barycenters cover ±9999 yr, and JPL's own manual recommends barycenters for osculating-element output. Giant-planet vs own-barycenter offset ≤ ~100 km — far under two-body display budgets. Requires: manifest route edit, ARCHITECTURE §5.3 wording (human edit), dry-run/spec text updates. Companion (non-controversial) hardening: dump the raw response on parse failure; add `--capture DIR` for the reproducibility commit. Confirmation probe + full analysis in brief §Q5. | 2026-07-12 | open — blocks WP3 online capture |

## Change log (append-only; newest first)

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
