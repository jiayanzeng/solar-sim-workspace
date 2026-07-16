# WP16 Steam overlay spike

This is the evidence sheet for ARCHITECTURE §11's real-client overlay risk.
App ID 480 is the human-approved **INTERIM** Spacewar development identity;
`docs/wp16-steam-bringup-decisions-2026-07-15.md` records its provenance and
the release guardrails.

## Status

| Platform | Real-client result | Current disposition |
|---|---|---|
| macOS / Metal, M2 Pro | Pending human run | Mac-first bring-up approved in Q13 |
| Windows / DX12 | Not run | Deferred under open Q13 until physical hardware exists |

The app treats the overlay as optional. A Steam initialization failure installs
the no-op `PlatformServices` adapter and continues with
`overlay_available=false`; successful initialization records the client's
reported overlay state without making rendering or simulation depend on it.

## macOS real-client commands

From the repository root on the M2 Pro:

```sh
open -a Steam
cargo build -p solar-sim --release --features steam
cargo run -p xtask -- prepare-steam-dev --app target/release/solar-sim
cargo run -p solar-sim --release --features steam
```

The generator overwrites `target/release/steam_appid.txt` from the committed
`STEAM_APP_ID` constant. Never create or edit that file by hand. The launch
must print one of these lines:

```text
steam: initialized app_id=480 overlay_available=true
steam: initialized app_id=480 overlay_available=false
```

An initialization-failure line means the real-client check has not run. Confirm
Steam is logged in as the same macOS user, then retry.

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
