# solar-sim — Rev C workspace (WP0–WP15 complete; WP16 in progress)

Steam-targeted solar-system simulator per `ARCHITECTURE.md` (Rev C, the design
of record). The full Bevy 0.19 application now runs: all 66 bodies propagate
from the committed real-ephemeris catalog, with the Eyes-modeled UI (time bar,
labels, left panel, search, layers), the BSC starfield, 2K KTX2 textures, the
settings screen with render recovery, and the golden-screenshot harness.
Remaining before beta: Steam release engineering (WP16, in progress) and the
QA/release gates (WP17).

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
  src/platform.rs     PlatformServices boundary (WP16, Steam adapter pending)
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

Mouse: drag to orbit the focused body, scroll to dolly (clamped between the
body surface and Sedna's aphelion). On-screen UI: time bar (detented rate
slider, editable date/clock, LIVE chip), search, breadcrumb, left panel
(Info / collections / View Options), layers quick panel, and the right rail.

Keyboard (see `crates/solar-sim/src/input_intent.rs` for the source of truth):

| Key | Action |
|---|---|
| `O` / `M` / `S` / `I` | travel to Sun / Mercury / Sedna / Io |
| `[` / `]` | step time rate down / up the ladder |
| `1` | real-time rate |
| `R` / `P` / `Space` | play / pause / toggle |
| `F9` | simulate device loss (debug builds only; exercises render recovery) |

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
  --simulate-device-loss         debug builds only
```

The macOS reference smoke check (also what CI runs on `macos-14`):

```
cargo run -p solar-sim --release -- --smoke 60 --expect-backend metal --reject-software-adapter
```

## Testing & verification

```
cargo test                                       # 201 tests, fully offline
cargo fmt --all -- --check                       # rustfmt defaults
cargo clippy --workspace --all-targets -- -D warnings
scripts/check-texture-metadata.sh                # texture license/hash audit
cargo run -p xtask -- gen-catalog --dry-run      # print the 66-body fetch plan (no network)
cargo run -p xtask -- gen-catalog \
    --fixtures xtask/fixtures --allow-partial \
    --out assets/catalog.sample.ron              # offline end-to-end (6 bodies; 60 skipped is expected)
```

The authoritative test baseline lives in `TASKS.md` (currently **201
passing**: 53 `sim-core` · 102 `solar-sim` · 43 `xtask` lib · 2 xtask smoke ·
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
reproducibility. Regeneration currently re-lints ~45 bodies for empty
`description` fields — a known content gap, not an error.

### Other xtask tools

```
cargo run -p xtask -- bake-starfield --source PATH --out PATH [--limit N]
cargo run -p xtask -- convert-texture --source PATH.ppm --out PATH.ktx2 [--alpha-from-luminance]
cargo run -p xtask -- check-texture-metadata [--dir assets/textures]
cargo run -p xtask -- capture-goldens --app PATH --out DIR --backend TAG
cargo run -p xtask -- compare-goldens --baseline DIR --candidate DIR [--max-mean F] [--max-p99 F]
```

## CI

`.github/workflows/ci.yml` runs on every push/PR:

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
| 16 | **in progress** — dependency-free `PlatformServices` boundary, mock lifecycle test, and the CI Steamworks guard landed. Blocked on Q14 (approve `steamworks = "0.13.1"` + App ID) before the feature-gated adapter, and on Q13 (real Windows hardware) for the Windows overlay spike and dev-branch install checks. |
| 17 | todo — replay suite, perf gates, demo script, licensing audit |
| 18 | deferred (Compare Size mode) |

Open questions awaiting the human: **Q4** (constellation line-set licensing,
fast-follow), **Q12** (CI-1…CI-6 brief scope), **Q13** (Windows/reference
hardware), **Q14** (Steamworks dependency + App ID). Details in
`TASKS.md → Open questions`.

## Known limitations

- No `steam` cargo feature exists yet; default builds have no Steamworks in
  the dependency tree (CI-enforced) and this stays true after WP16.
- Real-hardware Windows validation (launch, overlay, DX12 goldens, GTX 1650
  perf) is deferred to WP16/WP17 per Q10/Q13.
- ~45 catalog `description` fields are still empty (generator lints them on
  regeneration); the Info panel renders for all 66 bodies regardless.

## Non-negotiables carried from ARCHITECTURE §3

- `sim-core` must never depend on Bevy (CI-enforced).
- The app never touches the network; only `xtask --features online` does, at dev time.
- `assets/catalog.ron` is generated + committed; hand-editing it is a review-blocking offense.
- Everything is TDB / ecliptic-J2000 / km / degrees in the file — see
  `docs/wp3-gen-catalog-spec.md` §5 before touching units.
- All user actions go through the `SimCommand` queue; no direct state mutation from UI code.
