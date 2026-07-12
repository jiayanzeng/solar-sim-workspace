# WP0 close-out & local development bring-up — macOS (Apple silicon)

Target machine: MacBook Pro (M2 Pro), no Windows hardware available.
This guide closes every item under `TASKS.md → WP0 — remaining to close`,
explains how the Windows half of WP0's acceptance is handled without a
Windows machine, and finishes with the WP3 online-capture procedure
(Part B), since that also runs on this Mac.

Intended location in the repo: `docs/wp0-dev-setup-macos.md`.

---

## Part A — WP0

### A0. One-time machine prerequisites

```bash
# 1. Xcode Command Line Tools (linker, Metal toolchain)
xcode-select --install        # no-op if already installed

# 2. rustup-managed Rust (skip if `rustup --version` works)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 3. Test runner used by CI (and locally, it's faster)
cargo install cargo-nextest --locked
```

Nothing else is required on macOS: Bevy 0.19 uses Metal via wgpu, no SDK
downloads, no Vulkan runtime.

### A1. `rust-toolchain.toml` (closes TASKS Q1)

Evidence gathered 2026-07-12 from the crates.io API: **bevy 0.19.0
declares `rust_version = "1.95.0"`** (as do 0.19.0-rc.1 through rc.3).
ARCHITECTURE §8.1 says to pin the *minimum* stable that Bevy 0.19
supports, so at the repo root create:

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.95.0"
components = ["rustfmt", "clippy"]
```

Notes:
- Your shell prompt shows 1.96.0 installed; rustup will fetch and use
  1.95.0 automatically for this workspace the first time you build. Both
  satisfy Bevy's MSRV — the pin exists so every machine and CI runner
  agrees.
- `sim-core` keeps its conservative MSRV claim. Add to
  `crates/sim-core/Cargo.toml` under `[package]`:

  ```toml
  rust-version = "1.75"
  ```

  The workspace's Rust 1.95-generated v4 `Cargo.lock` cannot be parsed by
  Cargo 1.75. Verify the claim from a disposable standalone copy instead
  (the CI invariants job does the same):

  ```bash
  msrv_dir="$(mktemp -d)"
  cp -R crates/sim-core/. "$msrv_dir/"
  cargo +1.75.0 check --manifest-path "$msrv_dir/Cargo.toml"
  ```

After this lands, mark Q1 closed in `TASKS.md` with the crates.io
evidence.

### A2. Workspace `Cargo.toml` updates

```toml
[workspace]
resolver = "2"
members = ["crates/sim-core", "crates/solar-sim", "xtask"]

# Bevy is unusably slow at opt-level 0. Standard Bevy practice:
# light optimization for our code, full for dependencies.
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

Keep the existing comment about the sim-core/Bevy firewall; it stays true
and A4 makes it mechanical.

### A3. `crates/solar-sim` skeleton

The shell below is verified against Bevy 0.19.0. In particular, Bevy 0.19
uses `MessageReader` / `MessageWriter` for mouse input and `AppExit`; the
older `EventReader` / `EventWriter` names do not compile. The structure
(plugins, `--smoke`, the debug-only overlay, and the orbit-rig resource)
is WP0 scaffolding; WP5 replaces the direct-input stub with `SimCommand`
routing.

`crates/solar-sim/Cargo.toml`:

```toml
[package]
name = "solar-sim"
version = "0.1.0"
edition = "2021"

[dependencies]
sim-core = { path = "../sim-core" }
# "0.19" + committed Cargo.lock = pinned to an exact 0.19.x patch,
# patch upgrades are a deliberate `cargo update -p bevy` (ARCHITECTURE §8.1).
bevy = "0.19"

[features]
# Fast local iteration; NEVER enabled in CI or release builds.
dev = ["bevy/dynamic_linking"]
```

`crates/solar-sim/src/main.rs`:

```rust
//! WP0 — application shell (ARCHITECTURE §8): window, orbit-camera stub,
//! dev-only diagnostics overlay, and a `--smoke` mode for CI launch checks.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

/// Orbit rig stub (full input-intent layer + SimCommand routing is WP5 —
/// do NOT grow direct-input mutation habits here; this stub is throwaway
/// in exactly that respect).
#[derive(Resource)]
struct OrbitRig {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

impl Default for OrbitRig {
    fn default() -> Self {
        Self { yaw: 0.0, pitch: 0.35, distance: 12.0 }
    }
}

/// `--smoke N`: exit(0) after N rendered frames — used by CI to prove the
/// app launches and renders, not just links.
#[derive(Resource)]
struct SmokeFrames(Option<u32>);

fn main() {
    let smoke = std::env::args()
        .skip_while(|a| a != "--smoke")
        .nth(1)
        .and_then(|n| n.parse::<u32>().ok())
        .or(std::env::args().any(|a| a == "--smoke").then_some(60));

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "solar-sim (WP0 shell)".into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(OrbitRig::default())
    .insert_resource(SmokeFrames(smoke))
    .add_systems(Startup, setup)
    .add_systems(Update, (orbit_rig_stub, smoke_exit));

    #[cfg(debug_assertions)]
    {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .add_systems(Startup, spawn_diag_overlay)
            .add_systems(Update, update_diag_overlay);
    }

    app.run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Placeholder sun so there is something to look at until WP4.
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.3),
            emissive: LinearRgba::rgb(4.0, 3.2, 0.8),
            ..default()
        })),
    ));
    commands.spawn((Camera3d::default(), Transform::default()));
}

/// Right-drag orbits, scroll dollies. Raw input handling is acceptable
/// ONLY inside WP0's stub; WP5 replaces this with the SimCommand path.
fn orbit_rig_stub(
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut rig: ResMut<OrbitRig>,
    mut cam: Query<&mut Transform, With<Camera3d>>,
) {
    if buttons.pressed(MouseButton::Right) {
        for m in motion.read() {
            rig.yaw -= m.delta.x * 0.005;
            rig.pitch = (rig.pitch + m.delta.y * 0.005).clamp(-1.5, 1.5);
        }
    } else {
        motion.clear();
    }
    for w in wheel.read() {
        rig.distance = (rig.distance * (1.0 - w.y * 0.1)).clamp(2.0, 200.0);
    }
    if let Ok(mut t) = cam.single_mut() {
        let (sy, cy) = rig.yaw.sin_cos();
        let (sp, cp) = rig.pitch.sin_cos();
        t.translation = Vec3::new(cy * cp, sp, sy * cp) * rig.distance;
        t.look_at(Vec3::ZERO, Vec3::Y);
    }
}

fn smoke_exit(
    mut frames: Local<u32>,
    smoke: Res<SmokeFrames>,
    mut exit: MessageWriter<AppExit>,
) {
    if let Some(n) = smoke.0 {
        *frames += 1;
        if *frames >= n {
            exit.write(AppExit::Success);
        }
    }
}

#[cfg(debug_assertions)]
#[derive(Component)]
struct DiagText;

#[cfg(debug_assertions)]
fn spawn_diag_overlay(mut commands: Commands) {
    commands.spawn((Text::new("fps: --"), DiagText));
}

#[cfg(debug_assertions)]
fn update_diag_overlay(
    diags: Res<DiagnosticsStore>,
    mut q: Query<&mut Text, With<DiagText>>,
) {
    if let Some(fps) = diags
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
    {
        for mut t in &mut q {
            **t = format!("fps: {fps:.0}");
        }
    }
}
```

Bring-up:

```bash
cargo run -p solar-sim --features dev     # window + emissive sphere + fps
cargo run -p solar-sim -- --smoke 60      # renders 60 frames, exits 0
cargo test                                # still green (baseline in TASKS.md)
git add crates/solar-sim Cargo.lock && git commit
```

`Cargo.lock` MUST be committed from this point on — it *is* the exact
Bevy pin.

### A4. CI — `.github/workflows/ci.yml`

Covers: fmt, clippy (deny warnings), nextest, macOS + Windows builds, the
core-purity rule, and the offline rule.

```yaml
name: ci
on:
  push: { branches: [main] }
  pull_request:

env:
  CARGO_TERM_COLOR: always

jobs:
  lint:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.95.0
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets -- -D warnings

  test-macos:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.95.0
      - uses: Swatinem/rust-cache@v2
      - uses: taiki-e/install-action@nextest
      - run: cargo nextest run --workspace
      - name: offline smoke (fixtures pipeline)
        run: cargo run -p xtask -- gen-catalog --fixtures xtask/fixtures --allow-partial --out /tmp/catalog.smoke.ron

  build-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.95.0
      - uses: Swatinem/rust-cache@v2
      - uses: taiki-e/install-action@nextest
      - run: cargo nextest run --workspace
      - run: cargo build -p solar-sim --release
      # Launch check on a GPU-less runner: wgpu falls back to a software
      # DX12 adapter (WARP). Keep continue-on-error until proven stable,
      # then promote to a hard gate.
      - name: smoke launch (software adapter)
        continue-on-error: true
        run: ./target/release/solar-sim.exe --smoke 10

  invariants:
    runs-on: ubuntu-latest       # cheap; pure `cargo tree` checks
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.95.0
      - uses: dtolnay/rust-toolchain@1.75.0
      - name: sim-core MSRV — verify outside the v4 workspace lockfile
        run: |
          cp -R crates/sim-core "$RUNNER_TEMP/sim-core-msrv"
          cargo +1.75.0 check --manifest-path "$RUNNER_TEMP/sim-core-msrv/Cargo.toml"
      - name: core purity — sim-core must never depend on bevy
        run: |
          if cargo tree -p sim-core -e normal --prefix none | grep -qi '^bevy'; then
            echo '::error::core purity violated: sim-core depends on a bevy crate'
            cargo tree -p sim-core -e normal | grep -i bevy
            exit 1
          fi
      - name: offline rule — no network dep in default builds
        run: |
          for p in sim-core xtask solar-sim; do
            if cargo tree -p "$p" -e normal --prefix none | grep -Eqi '^(ureq|reqwest|hyper)'; then
              echo "::error::offline rule violated: $p pulls an HTTP client in its default build"
              exit 1
            fi
          done
      - name: offline rule — CI never builds the online feature
        run: |
          pattern='--features[ =]+on''line|features:.*on''line'
          if grep -REn --include='*.yml' -- "$pattern" .github/workflows/; then
            echo '::error::a workflow enables the online feature'
            exit 1
          fi
```

Two remarks:
- The invariants job encodes AGENTS.md rules 1–3 mechanically; if an
  agent ever adds a Bevy dep to `sim-core`, CI (not review) catches it.
- The Windows smoke launch is `continue-on-error` on purpose: WARP
  software rendering on GitHub runners is usually fine for wgpu but is
  not guaranteed. Watch it for a few runs; if it's reliably green, delete
  `continue-on-error` and it becomes your "window opens on Windows" gate.

### A5. Windows acceptance without a Windows machine

What CI gives you, in increasing strength:

1. **Compile + link on `windows-latest`** — catches all platform-cfg,
   linker, and dependency issues. This is the bulk of Windows risk at
   WP0's stage.
2. **`--smoke` launch on the runner's software adapter** — proves window
   creation, swapchain setup, and N rendered frames under DX12/WARP.
3. What CI *cannot* prove: behavior on real consumer GPUs/drivers.

Recommendation: treat 1 + 2 as WP0's Windows acceptance (the deferred
checkbox in `TASKS.md` records the residual), and plan real-hardware
verification before WP16 via any of: a friend's gaming PC running a
zipped build, a cheap cloud Windows box with GPU for an hour, or
Parallels on the M2 (note: Parallels runs *ARM* Windows — good for a
sanity launch via x64 emulation or an aarch64 build, but the Steam
depot target is `windows-x64`, so CI's x64 build stays the gate that
matters).

### A6. WP0 done — evidence to record in TASKS.md

- [ ] `rust-toolchain.toml` @ 1.95.0 committed; Q1 closed with the
      crates.io citation.
- [ ] `crates/solar-sim` in the workspace; `cargo run -p solar-sim`
      opens a window with the fps overlay on the Mac; `--smoke` exits 0.
- [ ] `Cargo.lock` committed.
- [ ] CI green: lint, test-macos, build-windows, invariants.
- [ ] Change-log entry citing the CI run URL and the local run.

---

## Part B — WP3 online capture (runs on this Mac)

Blocked on the **Q5 decision** (see
`docs/open-questions-brief-2026-07-12.md` §Q5 for the diagnosis of the
Jupiter `no $$SOE` failure and the proposed barycenter-route fix). Once
Q5 is signed off:

### B1. Confirm the diagnosis (30 seconds, optional but satisfying)

```bash
# Jupiter planet-center at the generator's 2300 sample. Horizons should
# report that Jupiter-center ephemerides stop in 2200, with no $$SOE:
curl -s "https://ssd.jpl.nasa.gov/api/horizons.api?format=text&COMMAND='599'&OBJ_DATA='NO'&MAKE_EPHEM='YES'&EPHEM_TYPE='ELEMENTS'&CENTER='500@10'&REF_PLANE='ECLIPTIC'&REF_SYSTEM='J2000'&OUT_UNITS='KM-S'&TLIST_TYPE='JD'&TLIST='2561120.0'" | head -40

# Same instant, Jupiter system barycenter — should return $$SOE records:
curl -s "https://ssd.jpl.nasa.gov/api/horizons.api?format=text&COMMAND='5'&OBJ_DATA='NO'&MAKE_EPHEM='YES'&EPHEM_TYPE='ELEMENTS'&CENTER='500@10'&REF_PLANE='ECLIPTIC'&REF_SYSTEM='J2000'&OUT_UNITS='KM-S'&TLIST_TYPE='JD'&TLIST='2561120.0'" | head -40
```

### B2. Land the Q5 changes (agent task after sign-off)

1. `xtask/src/manifest.rs`: giant-planet routes
   `HorizonsPlanet { command: "599" } → "5"`, `"699" → "6"`,
   `"799" → "7"`, `"899" → "8"` (Mercury–Mars stay on 199–499: the
   failed run proves they work, and minimal change is the rule).
2. Diagnostics hardening in `xtask/src/lib.rs` /
   `xtask/src/horizons.rs`: on any parse failure, write the raw body to
   `target/xtask-debug/<id>.response.txt` and include the path in the
   error. Never again debug a fetch blind.
3. `--capture DIR` flag: the `Http` fetcher writes every raw response to
   `DIR/<cache_key>.json`. This directory is committed (it is the
   "captured API responses for reproducibility" the WP3 checklist already
   requires), and doubles as a fixtures source for future offline runs.
4. Tests: `url_is_stable` untouched; add a manifest test asserting the
   four giant routes use barycenter commands; dry-run snapshot updated.
5. ARCHITECTURE §5.3 wording — **human edit**, agents don't touch it.

### B3. The capture run

```bash
cargo run -p xtask --features online -- gen-catalog --online \
    --capture xtask/fixtures/captured-2026-07 \
    --out assets/catalog.ron
cargo test                              # loader + smoke still green
git add assets/catalog.ron xtask/fixtures/captured-2026-07
git commit -m "WP3: real 66-body capture (Q5 routes), raw responses committed"
```

Expected leftovers: the three TNO-moon lookups (Dysnomia, Hiʻiaka,
Namaka) still error until the lookup route is implemented — run with
fixtures for those or land the lookup resolution first (WP3 checklist
item 2).

### B4. Spot-check vectors (activates the armed gate)

For each of the 10 bodies in ARCHITECTURE §5.6 (Mercury, Earth, Jupiter,
Sedna, Io, Triton, Phoebe, Nereid, Halley, 3I/ATLAS), request Horizons
**VECTORS**, *parent-centric*, ecliptic-J2000, at JD 2461042.0
(2026-01-01 12:00 TDB) and JD 2446471.0 (1986-02-09 12:00 TDB — noon, to
stay consistent with the catalog's noon-based epochs; record the exact
`jd_tdb` you used in the JSON regardless). Pattern:

```bash
# planet/dwarf/comet: center = Sun body center
CENTER="500@10"     # for Io use 500@599, Triton/Nereid 500@899, Phoebe 500@699
curl -s "https://ssd.jpl.nasa.gov/api/horizons.api?format=text&COMMAND='199'&OBJ_DATA='NO'&MAKE_EPHEM='YES'&EPHEM_TYPE='VECTORS'&CENTER='${CENTER}'&REF_PLANE='ECLIPTIC'&REF_SYSTEM='J2000'&OUT_UNITS='KM-S'&VEC_TABLE='2'&TLIST_TYPE='JD'&TLIST='2461042.0'"
```

Transcribe positions into
`xtask/fixtures/spotcheck/vectors.json` as
`[{ "id": "...", "jd_tdb": ..., "position_km": [x, y, z], "tol_km": ... }]`,
drop the freshly generated catalog beside it as
`xtask/fixtures/spotcheck/catalog.ron`, document the per-category
tolerances in `docs/wp3-gen-catalog-spec.md`, and confirm
`cargo test` shows `horizons_position_spot_check` *asserting*, not
skipping. Suggested starting tolerances (tighten after the first run
shows real residuals): planets low-1e4 km at-epoch / larger at 1986 via
secular fit; moons mid-1e3 km; SBDB dwarfs/asteroids 1e4–1e5 km; comets
loosest (non-gravitational forces are ignored by design).

---

## Warm-up patch — kill the `dead_code` warning properly

`time::UNIX_EPOCH_JD` is referenced only from a doc comment, hence the
warning. A use inside `#[cfg(test)]` alone does not fix normal library
builds, so make the Julian-date relationship the production definition:

```rust
const SECONDS_J2000_MINUS_UNIX: f64 =
    (J2000_JD_TDB - UNIX_EPOCH_JD) * DAY_S;
```

Then pin the noon-vs-midnight trap independently in the tests module:

```rust
#[test]
fn unix_epoch_jd_constant_is_consistent() {
    // Pins the noon-vs-midnight trap from the WP1 change log:
    // J2000 is JD 2451545.0 (noon); Unix epoch is JD 2440587.5 (midnight).
    let seconds_from_jd = (J2000_JD_TDB - UNIX_EPOCH_JD) * DAY_S;
    assert_eq!(seconds_from_jd, 946_728_000.0);
    assert_eq!(seconds_from_jd, SECONDS_J2000_MINUS_UNIX);
}
```

The baseline is now 72; record that evidence in the TASKS.md change log.
