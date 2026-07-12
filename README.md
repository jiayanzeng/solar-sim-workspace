# solar-sim — Rev B workspace (WP3 first cut)

Steam-ready solar-system simulator per `solar-system-simulator-prototype-architecture-rev-b.md`.
This checkout contains the **first concrete deliverable**: the `catalog.ron` schema with
load-time validation in `sim-core`, and the `xtask gen-catalog` generator with its spec.

```
crates/sim-core/      engine-agnostic core (ZERO Bevy deps — CI rule lands in WP0)
  src/catalog.rs      schema of record + validation + loader tests   [WP3 ✅]
  src/time.rs         SimClock, ±100 yr/s ladder, LIVE, calendar     [WP1 ✅]
  src/kepler.rs       elliptic + hyperbolic solvers, state_at()      [WP2 ✅]
xtask/                offline dev tooling, never shipped
  src/manifest.rs     curated 66-body manifest (identity/taxonomy/phys/routes)
  src/{horizons,sbdb,normalize,fetch,emit,lib,main}.rs   generator pipeline
  fixtures/           SYNTHETIC smoke fixtures (labeled; not flight data)
docs/wp3-gen-catalog-spec.md   the normative WP3 spec
ARCHITECTURE.md     design of record, Rev C — READ-ONLY for agents
TASKS.md            living status board — agents update per its protocol
AGENTS.md           agent rules (root; stricter nested copies in sim-core/ and xtask/)
assets/catalog.sample.ron      emitted by the offline smoke run
```

## Commands

```
cargo test                                            # 35 tests, fully offline
cargo run -p xtask -- gen-catalog --dry-run           # print the 66-body fetch plan
cargo run -p xtask -- gen-catalog \
    --fixtures xtask/fixtures --allow-partial \
    --out assets/catalog.sample.ron                   # offline end-to-end
cargo run -p xtask --features online -- gen-catalog --online --out assets/catalog.ron
                                                      # real capture (dev machine w/ JPL access)
```

## Status vs. Rev B §11

| WP | State |
|---|---|
| 0 | Workspace + members exist; Bevy pin, CI (fmt/clippy/nextest, core-purity rule), window/camera — **todo** |
| 1 | ✅ full ladder ±100 yr/s (24 detents, Eyes labels, symmetric-log slider mapping) · start-epoch config (fixed default JD 2461042 / live) · LIVE detection + eased snap · 1800–2300 clamp with transition events · 2050 high-confidence boundary · strict date/time parse + round-trip across leap rules |
| 2 | ✅ elliptic + hyperbolic solvers (Newton + guaranteed bisection fallback) · `state_at(orbit, μ, t)` in parent ecliptic frame · secular-rate application · fitted-mean-motion consistency · guards (parabolic, NaN, sign mismatch) · convergence sweeps e∈{0…0.97, 1.2…6} · retrograde (Triton/Phoebe) + Nereid fixtures · RK4 cross-validation on both branches |
| 3 | Schema ✅ · validation ✅ · generator + routes ✅ · offline smoke ✅ · **remaining:** online capture of the real 66-body file, TNO-moon lookup resolution, curated-value review (`TODO(review)` markers), position spot-checks — **harness now armed** (`xtask/tests/spotcheck.rs`), activates when the online run drops `fixtures/spotcheck/{catalog.ron,vectors.json}` |

## Non-negotiables carried from Rev B

- `sim-core` must never depend on Bevy (make it a CI check in WP0).
- The app never touches the network; only `xtask --features online` does, at dev time.
- `assets/catalog.ron` is generated + committed; hand-editing it is a review-blocking offense.
- Everything is TDB / ecliptic-J2000 / km / degrees in the file — see spec §5 before touching units.
