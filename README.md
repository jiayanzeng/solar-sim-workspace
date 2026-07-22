# solar-sim — Rev C workspace (WP0–WP15 complete; WP16 deferred)

Steam-targeted solar-system simulator per `ARCHITECTURE.md` (Rev C, the design
of record). The full Bevy 0.19 application now runs: all 66 bodies propagate
from the committed real-ephemeris catalog, with the Eyes-modeled UI (time bar,
labels, left panel, search, layers), the BSC starfield, 2K KTX2 textures, the
settings screen with render recovery, and the golden-screenshot harness.
Remaining before beta: deferred Steam release engineering (WP16) and the
dependent QA/release gates (WP17). Q13 records the unresolved hardware,
credentials, signing, and reference-machine prerequisites.

```
crates/sim-core/      engine-agnostic core (ZERO Bevy deps — CI-enforced)
  src/catalog.rs      schema of record + validation + loader          [WP3 ✅]
  src/time.rs         SimClock, ±100 yr/s ladder, LIVE, calendar      [WP1 ✅]
  src/kepler.rs       elliptic + hyperbolic solvers, state_at()       [WP2 ✅]
crates/solar-sim/     the Bevy 0.19 app                               [WP4–WP15 ✅]
  src/lib.rs          propagation, floating origin, schedule, CLI
  src/{control,input_intent}.rs   SimCommand queue, key map, replay
  src/{labels,orbit_lines,left_panel,layers,search,time_bar}.rs   Eyes UI
  src/{settings,scene_polish,starfield,surface_textures}.rs
  src/golden.rs       six canonical views, offscreen capture
  src/platform.rs     PlatformServices boundary + feature-gated Steam adapter
xtask/                offline dev tooling, never shipped
  src/manifest.rs     curated 66-body manifest (human-approved values + provenance)
  src/{horizons,sbdb,normalize,fetch,emit,lookup}.rs   catalog pipeline
  src/{starfield,texture,golden}.rs                    asset + golden tooling
  fixtures/           captured JPL responses (2026-07) + synthetic smoke fixtures
assets/catalog.ron    committed real 66-body catalog (generated — NEVER hand-edit)
assets/textures/      2K KTX2 set with per-file public-domain license sidecars
assets/starfield.bsc  baked 5,000-star NASA HEASARC BSC5P asset
docs/                 WP specs, audits, golden-screenshot doc, open-question briefs
ARCHITECTURE.md       design of record, Rev C — READ-ONLY for agents
TASKS.md              living status board — agents update per its protocol
AGENTS.md             agent rules (root; stricter nested copies in sim-core/ and xtask/)
```

## Quick start — run the game

```
cargo run -p solar-sim --release
```

That is the whole thing: `rust-toolchain.toml` pins Rust 1.95.0 automatically,
`Cargo.lock` pins Bevy 0.19.x exactly, and the committed `assets/catalog.ron`
is loaded and validated at startup. Debug builds work too, but use `--release`
for smooth frame rates. The app starts at the configured 2026 epoch (JD
2461042.0 TDB) unless settings say otherwise. It never touches the network.

### In-app controls

Mouse: left-drag or right-drag the scene to orbit the focused body; a short
primary click still selects. Scroll to dolly (clamped between the body surface
and Sedna's aphelion). On-screen UI: time bar (detented rate
slider, editable date/clock, LIVE chip), search, breadcrumb, left panel
(Info / collections / View Options), layers quick panel, and the right rail.

Keyboard (see `crates/solar-sim/src/input_intent.rs` for the source of truth):

| Key | Action |
|---|---|
| `O` / `M` / `S` / `I` | travel to Sun / Mercury / Sedna / Io |
| `←` / `→` or `[` / `]` | step time rate down / up the signed ladder |
| `↓` | select +1 day/s without changing play/pause state |
| `1` / `2` / `3` / `4` | travel to the Inner / Belt / Outer / Kuiper region preset |
| `R` / `P` / `Space` | play / pause / toggle |
| `Home` | reset to the Sun-focused startup angle and full-system framing |
| `Escape` | revert text, close the active modal, or open the controls guide from the scene |
| `F9` | simulate device loss (debug builds only; exercises render recovery) |
| `F10` | toggle the frame-time overlay (debug builds only; hidden by default) |

Open Settings from the right rail. Its 39 controls accept pointer input, the
content area scrolls independently of the camera, `REVERT` restores the current
persisted values, `RESTORE DEFAULTS` explicitly resets the reviewed product and
presentation defaults, and both `APPLY` and `CLOSE` dismiss the modal. Gameplay
input is suppressed while Settings or Browse is open and while a text field is
being edited.

Persisted layer choices still round-trip exactly. If UI is visible but Orbits,
Labels, and Icons are all disabled, a transient `RESTORE DEFAULT VIEW` notice
provides a one-action recovery path without silently changing the settings
file. If UI itself is disabled, the existing `SHOW UI` control remains the
recovery surface.

### App CLI flags

```
cargo run -p solar-sim --release -- [flags]
  --catalog PATH                 alternate catalog file (default assets/catalog.ron)
  --focus ID                     start focused on a body (e.g. --focus jupiter)
  --smoke [N]                    render N frames (default 60) and exit 0 — CI launch check
  --expect-backend metal|dx12|vulkan   with --smoke: fail if wgpu picked another backend
  --reject-software-adapter      with --smoke or golden capture: fail on WARP/llvmpipe
  --assert-nonblack              with --smoke: fail if the frame is entirely black
  --golden-view V --golden-backend B --golden-capture DIR   capture one canonical view
  --frame-stats SECONDS --frame-stats-out PATH   measure CPU frame time and write JSON + PATH.csv
  --frame-stats-view V          transient canonical-view selector for reproducible measurements
  --frame-stats-quality P       transient low|medium|high|ultra measurement override
  --reset-settings               persist reviewed defaults before normal startup
  --simulate-device-loss         debug builds only
```

The macOS reference smoke check (also what CI runs on `macos-14`):

```
cargo run -p solar-sim --release -- --smoke 60 --expect-backend metal --reject-software-adapter
```

## Testing & verification

```
cargo test                                       # 398 tests, fully offline
cargo fmt --all -- --check                       # rustfmt defaults
cargo clippy --workspace --all-targets -- -D warnings
scripts/check-texture-metadata.sh                # texture license/hash audit
cargo run -p xtask -- gen-catalog --dry-run      # print the 66-body fetch plan (no network)
cargo run -p xtask -- gen-catalog \
    --fixtures xtask/fixtures --allow-partial \
    --out assets/catalog.sample.ron              # offline end-to-end (6 bodies; 60 skipped is expected)
cargo run -p xtask -- perf-report target/perf/*.json  # format WP17 evidence table
```

The authoritative test baseline lives in `TASKS.md` (currently **398
passing**: 53 `sim-core` · 288 `solar-sim` · 54 `xtask` lib · 2 xtask smoke ·
1 position spot-check gate, **active**). If this README and `TASKS.md`
disagree, `TASKS.md` wins.

### Regenerating the catalog (dev machines with JPL access only)

```
cargo run -p xtask --features online -- gen-catalog --online --out assets/catalog.ron
```

Q5 is closed and implemented: Mercury–Mars fetch geometric centers,
Jupiter–Neptune fetch system barycenters, so the old `$$SOE` failure at
Jupiter is gone. The 2026-07-13 live run captured all 66 bodies; the raw
responses are committed under `xtask/fixtures/captured-2026-07/` for
reproducibility. All 66 bodies have curated descriptions and authoritative
content provenance; manifest and application regressions enforce completeness.

### Other xtask tools

```
cargo run -p xtask -- bake-starfield --source PATH --out PATH [--limit N]
cargo run -p xtask -- convert-texture --source PATH.ppm --out PATH.ktx2 [--alpha-from-luminance]
cargo run -p xtask -- check-texture-metadata [--dir assets/textures]
cargo run -p xtask -- capture-goldens --app PATH --out DIR --backend TAG
cargo run -p xtask -- compare-goldens --baseline DIR --candidate DIR [--max-mean F] [--max-p99 F]
cargo run -p xtask -- prepare-steam-dev --app target/release/solar-sim
cargo run -p xtask -- steam-release-preflight --action package|depot
```

The Mac-first real-client overlay procedure is in
`docs/wp16-steam-overlay-spike.md`. App ID 480 is development-only; packaging
and depot preflights reject it.

## CI

`.github/workflows/ci.yml` runs on pushes to `main` and on pull requests:

- **lint** — fmt, clippy (deny warnings), texture metadata audit
- **test-linux** — nextest full workspace + offline fixtures smoke
- **platform** (`macos-14`, `windows-latest`) — full tests, release build,
  smoke launch. The macOS smoke is a hard Metal gate; the Windows smoke runs
  on WARP (hosted runners have no GPU) and stays `continue-on-error` — it is
  a code-path check, not a launch verification.
- **invariants** — `sim-core` MSRV (1.75) check, core purity (no `bevy*` in
  `sim-core`'s tree), offline rule (no HTTP client in default builds; no
  `online` feature in any workflow), and the default-build Steamworks guard.

`.github/workflows/goldens.yml` (manual dispatch) builds release, captures the
six canonical views twice per backend, and enforces the perceptual stability
gate. Per Q10, the blocking stability platform is macOS/Metal; DX12/WARP
captures are non-blocking.

## Status vs. ARCHITECTURE §11 work packages

| WP | State |
|---|---|
| 0–15 | ✅ done — see `TASKS.md → Done (evidence)` and the change log |
| 16 | deferred — `PlatformServices`, the optional `steamworks` adapter, interim App ID 480 guardrails, and the default-build Steamworks CI guard are implemented. Q13 still blocks the remaining hardware, credentials, signing, packaging, and install evidence. |
| 17 | todo — replay suite, perf gates, demo script, licensing audit |
| 18 | deferred (Compare Size mode) |

Open questions awaiting the human: **Q4** (constellation line-set licensing,
fast-follow), **Q12** (CI-1…CI-6 brief scope), **Q13**
(Windows/reference hardware and credentials), and **Q18** (the exact WP17
nonblack-readback gate). Details in `TASKS.md → Open questions`.

## Known limitations

- The `steam` cargo feature is opt-in; default builds have no Steamworks in
  the dependency tree (CI-enforced). App ID 480 cannot pass release preflight.
- Real-hardware Windows validation (launch, overlay, DX12 goldens, GTX 1650
  perf) is deferred to WP16/WP17 per Q10/Q13.

## Non-negotiables carried from ARCHITECTURE §3

- `sim-core` must never depend on Bevy (CI-enforced).
- The app never touches the network; only `xtask --features online` does, at dev time.
- `assets/catalog.ron` is generated + committed; hand-editing it is a review-blocking offense.
- Everything is TDB / ecliptic-J2000 / km / degrees in the file — see
  `docs/wp3-gen-catalog-spec.md` §5 before touching units.
- All user actions go through the `SimCommand` queue; no direct state mutation from UI code.
