# WP15 golden screenshots

The renderer owns six canonical views at a fixed 960 × 600 logical-pixel
window and a 1.0 scale factor. `crates/solar-sim/src/golden.rs` is the source of
truth for camera poses and layer profiles; this document gives reviewers the
intent behind those values. The persisted user settings file is ignored while
capturing. Simulation time starts at the catalog's fixed default epoch. The 3D
camera renders to a fixed-size sRGB image target so capture does not depend on
swapchain visibility or window-surface state; Metal/DX12 still render every
pixel through their normal backend pipelines.

| View | Focus | Review target |
| --- | --- | --- |
| `full-system` | Sun | Product HUD, labels, category colors, orbit-line palette, and system framing |
| `inner-orbits` | Sun | Parent-relative inner paths and high-density orbit geometry from an elevated angle |
| `earth-texture` | Earth | 2K globe mapping, seam/pole behavior, day/night contrast, and catalog-color-independent detail |
| `jupiter-system` | Jupiter | Major-moon context, labels/reticles, and parent-relative moon orbits |
| `saturn-rings` | Saturn | Textured translucent annulus, inner gap, outer edge, and two-sided oblique rendering |
| `sun-bloom` | Sun | Emissive texture, bloom falloff, starfield contrast, and low ambient level |

The full-system view retains the product HUD. The five scene-detail views
remove HUD surfaces in the capture-only path; this does not alter `LayerState`
or the application's UI-off behavior. Planet and ring views wait for every
referenced image asset and for five seconds of render-pipeline settling before
readback. Some backends specialize the final render-target pipeline on the
first screenshot request; an all-black readback is rejected and retried in the
same process up to three times after a two-second settle. It is never accepted
as a golden, and any final capture error exits non-zero.

## Capture and comparison

Build the application once, then use the offline xtask harness:

```sh
cargo build -p solar-sim --release
cargo run -p xtask -- capture-goldens --app target/release/solar-sim --out target/goldens/run-a --backend metal
cargo run -p xtask -- capture-goldens --app target/release/solar-sim --out target/goldens/run-b --backend metal
cargo run -p xtask -- compare-goldens --baseline target/goldens/run-a/metal --candidate target/goldens/run-b/metal
```

Windows CI uses the same commands with `solar-sim.exe` and the `dx12` backend.
The application writes strict binary PPM (`P6`) files so comparison needs no
image-codec dependency. The default gate computes CIE Lab Delta E 76 and
requires mean Delta E ≤ 1.25 and the 99th percentile ≤ 4.0 for every view.
Both capture directories must contain exactly the six names above.

The launcher replaces Cargo's inherited `CARGO_MANIFEST_DIR` with the
`solar-sim` crate directory before starting the app. This keeps Bevy's
`../../assets` root anchored to the workspace assets when the launcher itself
is executed through `cargo run -p xtask`.

CI captures two independent application launches per backend, compares them,
and uploads the second set as the reviewed backend artifact. A deliberately
approved visual change is promoted by downloading those artifacts after both
backend jobs pass; thresholds are not loosened to accept a regression.

## Window-surface smoke check

Golden capture deliberately targets a fixed offscreen image, so it does not
replace a shipped-window check. On local or real release hardware, opt into a
primary-window readback after the smoke frame count:

```sh
cargo run -p solar-sim --release -- --smoke 60 --expect-backend metal --assert-nonblack
```

Use `dx12` on the Windows reference machine and `vulkan` for a Vulkan release
target. `--expect-backend` rejects an unexpected wgpu adapter backend, while
`--reject-software-adapter` rejects `wgpu::DeviceType::Cpu`. Any future real
DX12 machine must pass `cargo run -p solar-sim --release -- --smoke 60
--expect-backend dx12 --reject-software-adapter` before its golden captures
mean anything. The Windows hosted smoke deliberately omits this check because
its WARP adapter is an expected compile/launch probe, not a golden GPU gate.
`--assert-nonblack` reads the primary window render target and fails if every
RGB channel is zero. The nonblack assertion is intentionally not a hosted-CI
gate: a hosted window surface may read back black even when the fixed offscreen
golden target is valid. WP17 runs this opt-in check on both real reference
machines before release closeout.
