# WP16 Steam overlay spike

This is the evidence sheet for ARCHITECTURE §11's real-client overlay risk.
App ID 480 is the human-approved **INTERIM** Spacewar development identity;
`docs/wp16-steam-bringup-decisions-2026-07-15.md` records its provenance and
the release guardrails.

## Status

| Platform | Real-client result | Current disposition |
|---|---|---|
| macOS / Metal, M2 Pro | Initial run failed; corrected retest pending | Mac-first bring-up approved in Q13 |
| Windows / DX12 | Not run | Deferred under open Q13 until physical hardware exists |

The app treats the overlay as optional. A Steam initialization failure installs
the no-op `PlatformServices` adapter and continues with
`overlay_available=false`; successful initialization records the client's
reported overlay state without making rendering or simulation depend on it.
Steam callbacks are pumped every frame, and the status resource follows the
overlay from its documented initial `false` state to `true` after injection.

## 2026-07-16 initial macOS result

The first M2 Pro run used macOS 26.5.1 and a signed-in Steam client whose build
number was not recorded. Steam API initialization succeeded for App ID 480, but
the app printed `overlay_available=false` and Shift-Tab was unresponsive. That
run is not acceptance evidence: the application initialized Steam after Bevy
had already created the Metal adapter, never pumped Steam callbacks, sampled
overlay availability only once, and the linker-signed executable had neither
macOS overlay entitlement.

The corrected development path initializes Steam before Bevy rendering,
refreshes overlay availability after callbacks, and has `xtask` ad-hoc sign the
binary with Valve's required
`com.apple.security.cs.disable-library-validation` and
`com.apple.security.cs.allow-dyld-environment-variables` entitlements. Valve
documents both the initialization-order and entitlement requirements in its
[overlay guide](https://partner.steamgames.com/doc/features/overlay) and
[macOS platform guide](https://partner.steamgames.com/doc/store/application/platforms).
An automated corrected launch remained at `overlay_available=false` for 15
seconds. `vmmap` showed the Steam API and client libraries loaded but no Steam
overlay renderer injected, so the code-side preconditions are now present but
the client-side global/per-Spacewar overlay setting and launch injection still
need the human UI check below. The macOS spike cannot be recorded as passing
until that retest succeeds.

## Validated application baseline before the overlay retest

The overlay investigation briefly exposed a separate Settings-screen
regression. That regression is no longer part of the overlay risk: on
2026-07-16 the human validated the normal non-Steam release build at commit
`60a19a6718edbc3b239606325f1b663c723d5a12`. Pointer adjustment of Settings,
scrolling, `REVERT`, `APPLY`, `CLOSE`, and Escape all worked in the real macOS
window. Hosted
[CI run 29488349896](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29488349896)
passed `lint`, `test-linux`, `invariants`, `platform (macos-14)`, and `platform
(windows-latest)` for the same commit.

This usability result is only a preflight baseline. It does not establish
overlay injection or change either platform row in the status table.

## macOS real-client commands

From the repository root on the M2 Pro:

```sh
open -a Steam
cargo build -p solar-sim --release --features steam
cargo run -p xtask -- prepare-steam-dev --app target/release/solar-sim
cargo run -p solar-sim --release --features steam
```

The generator overwrites `target/release/steam_appid.txt` from the committed
`STEAM_APP_ID` constant and ad-hoc signs the macOS executable with the
development overlay entitlements. Never create or edit the marker by hand.
Steam's global in-game overlay and Spacewar's per-game overlay must both be
enabled before launching.

The `[S_API] SteamAPI_Init()` line must appear before Bevy's Metal
`AdapterInfo` line. The initialization line may initially report false:

```text
steam: initialized app_id=480 overlay_available=false
platform: overlay_available=true
```

Valve documents that `IsOverlayEnabled` can remain false for several seconds
while injection finishes. Wait for the `platform: overlay_available=true`
transition before testing Shift-Tab. An initialization-failure line means the
real-client check has not run; confirm Steam is logged in as the same macOS
user, then retry.

With Steam's in-game overlay enabled, press Shift-Tab and record whether the
overlay appears over the Metal window. Then disable the overlay for Spacewar in
Steam, relaunch with the same command, and confirm that Solar Sim remains usable
when the terminal reports `overlay_available=false`.

## Human result to return

Record the macOS version, Steam client build, both exact `steam:` output lines,
whether Shift-Tab opened the overlay, and whether the app continued to render
and accept input with the overlay disabled. Those observations will be appended
here; this document does not claim the macOS or Windows spike before the real
desktop checks occur.

## Release identity guard

Every future packaging and depot entrypoint must call the shared preflight:

```sh
cargo run -p xtask -- steam-release-preflight --action package
cargo run -p xtask -- steam-release-preflight --action depot
```

Both commands intentionally fail while `STEAM_APP_ID` is 480. They may pass
only after the partner account exists and the human approves replacing the
interim constant with Solar Sim's real App ID.
