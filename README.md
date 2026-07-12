# solar-sim — Rev C workspace (WP1–WP3 core complete)

Steam-ready solar-system simulator per `ARCHITECTURE.md` (Rev C, the design of
record). This checkout contains the shipped-and-tested core: `sim-core`
(catalog schema + validation, `SimClock`, elliptic/hyperbolic Kepler) and the
`xtask gen-catalog` generator with its spec.

```
crates/sim-core/      engine-agnostic core (ZERO Bevy deps — CI rule lands in WP0)
  src/catalog.rs      schema of record + validation + loader tests   [WP3 ✅]
  src/time.rs         SimClock, ±100 yr/s ladder, LIVE, calendar     [WP1 ✅]
  src/kepler.rs       elliptic + hyperbolic solvers, state_at()      [WP2 ✅]
crates/solar-sim/     the Bevy 0.19 app                              [WP0 — see docs/wp0-dev-setup-macos.md]
xtask/                offline dev tooling, never shipped
  src/manifest.rs     curated 66-body manifest (identity/taxonomy/phys/routes)
  src/{horizons,sbdb,normalize,fetch,emit,lib,main}.rs   generator pipeline
  fixtures/           SYNTHETIC smoke fixtures (labeled; not flight data)
docs/wp3-gen-catalog-spec.md       the normative WP3 spec
docs/wp0-dev-setup-macos.md        local dev bring-up + WP0 close-out guide
docs/open-questions-brief-2026-07-12.md   research briefs for TASKS.md open questions
ARCHITECTURE.md     design of record, Rev C — READ-ONLY for agents
TASKS.md            living status board — agents update per its protocol
AGENTS.md           agent rules (root; stricter nested copies in sim-core/ and xtask/)
assets/catalog.sample.ron      emitted by the offline smoke run (6 bodies)
```

## Commands

```
cargo test                                            # 71 tests, fully offline
cargo run -p xtask -- gen-catalog --dry-run           # print the 66-body fetch plan
cargo run -p xtask -- gen-catalog \
    --fixtures xtask/fixtures --allow-partial \
    --out assets/catalog.sample.ron                   # offline end-to-end (6 bodies; 60 skipped is expected)
cargo run -p xtask --features online -- gen-catalog --online --out assets/catalog.ron
                                                      # real capture (dev machine w/ JPL access) — see Known issues
```

The authoritative test baseline lives in `TASKS.md` (currently **71
passing**: 51 `sim-core` · 17 `xtask` lib · 2 smoke · 1 spot-check harness,
dormant). If this README and `TASKS.md` disagree, `TASKS.md` wins.

## Known issues

- **`--online` capture currently fails at Jupiter** with `no $$SOE in
  Horizons result`. Root cause analysis and the proposed fix (switch the
  giant-planet routes from planet centers 599/699/799/899 to system
  barycenters 5/6/7/8) are in `TASKS.md → Open questions → Q5` and
  `docs/open-questions-brief-2026-07-12.md`. The route change touches the
  curated manifest and ARCHITECTURE §5.3 wording, so it needs human
  sign-off before an agent applies it.
- **`dead_code` warning on `time::UNIX_EPOCH_JD`.** The constant is
  referenced only from a doc comment. Planned fix: a consistency test
  pinning `(J2000_JD_TDB − UNIX_EPOCH_JD) · 86400 = SECONDS_J2000_MINUS_UNIX`
  (the noon-vs-midnight trap), which uses the constant and raises the
  baseline to 72. Patch in `docs/wp0-dev-setup-macos.md` §Warm-up.

## Status vs. ARCHITECTURE §11 work packages

| WP | State |
|---|---|
| 0 | Workspace + members exist; **remaining:** `rust-toolchain.toml` (pin **1.95.0** — Bevy 0.19.0's declared MSRV, see TASKS.md Q1), `crates/solar-sim` skeleton, CI (fmt/clippy/nextest, core-purity rule, offline rule, macOS + Windows), window/camera/diagnostics. Full walkthrough: `docs/wp0-dev-setup-macos.md` |
| 1 | ✅ full ladder ±100 yr/s (24 detents, Eyes labels, symmetric-log slider mapping) · start-epoch config (fixed default JD 2461042 / live) · LIVE detection + eased snap · 1800–2300 clamp with transition events · 2050 high-confidence boundary · strict date/time parse + round-trip across leap rules |
| 2 | ✅ elliptic + hyperbolic solvers (Newton + guaranteed bisection fallback) · `state_at(orbit, μ, t)` in parent ecliptic frame · secular-rate application · fitted-mean-motion consistency · guards (parabolic, NaN, sign mismatch) · convergence sweeps e∈{0…0.97, 1.2…6} · retrograde (Triton/Phoebe) + Nereid fixtures · RK4 cross-validation on both branches |
| 3 | Schema ✅ · validation ✅ · generator + routes ✅ · offline smoke ✅ · **remaining:** online capture of the real 66-body file (blocked on Q5 route decision), TNO-moon lookup resolution, curated-value review (`TODO(review)` markers — research brief ready), position spot-checks — **harness armed** (`xtask/tests/spotcheck.rs`), activates when the online run drops `fixtures/spotcheck/{catalog.ron,vectors.json}` |
| 4–18 | Per `TASKS.md` — detailed briefs with acceptance criteria are in `TASKS.md → Work package briefs` |

## Non-negotiables carried from ARCHITECTURE §3

- `sim-core` must never depend on Bevy (becomes a CI check in WP0).
- The app never touches the network; only `xtask --features online` does, at dev time.
- `assets/catalog.ron` is generated + committed; hand-editing it is a review-blocking offense.
- Everything is TDB / ecliptic-J2000 / km / degrees in the file — see spec §5 before touching units.
