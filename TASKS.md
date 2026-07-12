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

---

## Dashboard

| WP | Deliverable | Status |
|---|---|---|
| 0 | Workspace, Bevy 0.19 pin, CI, window+camera+diagnostics, core-purity rule | **in-progress** (workspace + crates exist; Bevy app, toolchain pin, CI absent) |
| 1 | `sim-core::time` — full ladder, start epoch, LIVE, range | **✅ done** |
| 2 | `sim-core::kepler` — elliptic + hyperbolic, guards | **✅ done** |
| 3 | `xtask gen-catalog` + committed 66-body `catalog.ron` + validation | **in-progress** (pipeline ✅; real capture + review pending) |
| 4 | Propagation + floating origin: 66 colored spheres at 2026 positions | todo (unblocked by WP0) |
| 5 | Camera rig, input-intent layer, key map, travel tween | todo |
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
an accompanying change-log justification is a regression.

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
  responses for reproducibility. `blocked(network)` in sandboxed
  environments — a dev machine or Claude Code session can do it.
- [ ] **TNO moon resolution**: Horizons lookup-API resolution for
  Dysnomia / Hiʻiaka / Namaka COMMANDs and Eris/Haumea center designators
  (`xtask/src/lib.rs` lookup route; spec §8 item 1). Until then: fixtures.
- [ ] **Curated review pass**: clear every `TODO(review)` in
  `xtask/src/manifest.rs` (all radii; GMs for Pluto 869.6 / Eris 1108 /
  Haumea 267 km³/s²; 3I/ATLAS nucleus radius — pick a value, cite it).
  Human sign-off required; agents prepare the diff + citations only.
- [ ] **Spot-check activation**: capture Horizons VECTORS for the 10-body
  set (ARCHITECTURE §5.6) at JD 2461042.0 and 1986-02-09 into
  `xtask/fixtures/spotcheck/vectors.json`; document per-category
  tolerances in `docs/wp3-gen-catalog-spec.md`; `cargo test` must show the
  gate passing (not skipping).

## WP0 — remaining to close

- [ ] `rust-toolchain.toml` pinning the minimum stable Rust that Bevy
  0.19.x supports (note: `sim-core` currently compiles on 1.75; keep its
  MSRV conservative).
- [ ] `crates/solar-sim` skeleton: Bevy 0.19.x pinned exactly in
  `Cargo.toml`, `Cargo.lock` committed; window opens; orbit camera stub;
  dev-only `DiagnosticsOverlay`.
- [ ] CI (GitHub Actions or equivalent): fmt, clippy (deny warnings),
  nextest, macOS + Windows build jobs, **core-purity rule** (fail if
  `sim-core`'s dependency tree contains any `bevy*` crate), offline rule
  (no `online` feature in CI builds).
- [ ] Acceptance: app opens on macOS & Windows; CI green.

## Next up (dependency order)

1. **WP4** — propagation + floating origin. All 66 bodies as colored
   spheres at the 2026-01-01 configuration; moon states composed
   parent-centric onto parent heliocentric (use `kepler::state_at` +
   `dot/cross/norm`); origin rebase at camera focus; acceptance: planet
   longitudes eyeball-checked vs Horizons; no jitter focused on Mercury or
   Sedna. Depends on WP0 + real `catalog.ron` (sample catalog OK for
   development, not acceptance).
2. **WP5** — camera + input-intent + replay hashing (determinism suite
   starts here and never leaves CI).
3. **WP6** — orbit lines incl. the 3I/ATLAS ±25-yr hyperbolic arc.
4. **WP7/WP8** — ui_kit, then the time bar binding WP1's API
   (`RateIndex::detents`, `slider_pos`, `parse_date/parse_time`,
   `format_date_eyes`, `TickReport` toasts).
5. WP9–WP15 per ARCHITECTURE §§9–10; WP16–17 release engineering.

## Open questions (humans close these)

| # | Question | Raised | Status |
|---|---|---|---|
| Q1 | Confirm Bevy 0.19.x minimum Rust toolchain and record in WP0 pin | 2026-07-12 | open |
| Q2 | TNO GM values (Pluto 869.6 / Eris 1108 / Haumea 267 km³/s²) — accept or replace with cited values during curated review? | 2026-07-12 | open |
| Q3 | 3I/ATLAS nucleus radius: literature is uncertain; which value + citation ships? | 2026-07-12 | open |
| Q4 | Constellation-figure line set licensing (fast-follow; Yale BSC-derived in-house vs licensed) | 2026-07-12 | open |

## Change log (append-only; newest first)

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
