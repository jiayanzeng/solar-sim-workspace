# Project status — verification and evaluation (2026-07-22)

**Reviewer:** Claude (technical advisor / quality gate). **Source of record:** the
repomix bundle at project head, `TASKS.md` change log, `ARCHITECTURE.md` Rev C,
CI workflows, and all `docs/` audits. This is a verification of recorded
evidence, not a re-execution of the suites.

## 1. What "CI complete and actual testing done" verifiably means

The claim is **true at the integration level** and the evidence is coherent:

1. The four-phase UI/gameplay follow-up stack merged to `main` in dependency
   order on 2026-07-22 — PR #7 camera/discoverability (`df6e0a2`), PR #8
   selection interaction (`25e4a96`), PR #9 HUD polish (`ff1ef9f`), PR #10
   descriptions/provenance (`25ca288`). Each PR was retargeted onto current
   `main`, reported CLEAN/MERGEABLE, and passed all five hosted checks (lint,
   Linux, macOS, Windows, invariants). No open PR remains from the stack.
2. The projection-integrity (WP9), WP5, WP6, WP8/WP11, WP14-relaunch, and WP10
   phases each closed with the full local matrix green: workspace tests, Steam-
   feature tests, fmt, warning-denied clippy (both feature configurations),
   texture-metadata audit, catalog dry-run, `git diff --check`.
3. The test baseline is **369 default / 370 with `--features steam`**
   (53 sim-core · 264 solar-sim · 49 xtask lib · 2 smoke · 1 active
   spot-check), with the board's only-goes-up rule intact.
4. The 2026-07-18 documentation inconsistencies (223-test headline, WP16
   status conflict, CI-trigger description, branch-head evidence gap) were all
   resolved by the `codex/docs-status-cleanup` package and by the merges
   themselves. The earlier audit's "not integrated into main, not
   hosted-CI-certified at head" finding **no longer applies**.
5. Real-GPU evidence on the M2 Pro exists for every recent phase: nonblack
   Metal golden captures, and opt-in smoke runs at 92.2 fps (5120×2880,
   selection phase) and 120.0 fps (3456×2168, HUD phase — note this reading is
   the default `FrameCap::Fps120` cap, not a ceiling).

Conclusion for §1: the source baseline is integrated, hosted-certified at
head, and locally green across every defined gate. Through the WP0–WP15 +
corrective-cycle scope, the project is in its healthiest recorded state.

## 2. What it does not mean

The project is **not release-ready**, and the gap is exactly the previously
recorded hardware/credential surface, not code health:

- **WP16 (deferred).** Only the default-build Steamworks isolation box is
  checked. Outstanding: corrected macOS overlay retest (the entitlement-signed
  automated launch still reported `overlay_available=false`; not acceptance
  evidence), Windows overlay test, real App ID + partner account, Apple
  Developer ID + protected secrets, sign/notarize/staple dry-run, SteamPipe
  depots, both-OS dev-branch install, ≤150 MB bundle measurement.
- **WP17 (todo).** Demo script, replay library in CI, both-reference-machine
  ≥60 fps evidence, real-hardware Metal/DX12 smoke gates, signed licensing
  audit. Neither reference machine (M1 Air, GTX 1650-class) is owned.
- **Q18 (open).** The exact WP17 60-frame Metal smoke gate cannot currently be
  claimed on the M2 Pro: the one-shot primary-window readback returns black
  even when an immediately preceding golden capture (which uses the harness's
  texture-readiness wait) is nonblack, and 120 update frames were shown not to
  be a reliable readiness condition. This is a readback-readiness defect in the
  gate procedure, not a rendering defect. Ruling and fix are in the companion
  decision record.
- **Perf gates have never been measured.** The recorded fps numbers are
  incidental smoke outputs on an M2 Pro, partially frame-capped; no all-layers
  frame-time capture exists for any reference-class machine at any quality
  preset. The 60 fps Definition-of-Done item is unevidenced in either
  direction.

## 3. Open items inventory (post-integration)

| Item | Status | Disposition |
|---|---|---|
| Q4 constellation licensing | open | closed by delegated ruling — see decision record |
| Q12 CI-1…CI-6 briefs | open | closed as superseded — see decision record |
| Q13 hardware/credentials | open (hardware half) | purchase recommendation + final-stage plan — see decision record |
| Q18 readback gate | open | ruled — readiness precondition spec'd; implementation block UIP-8 |
| Ruling queue #2/#6/#8/#9 | awaiting rulings | ruled — see UI/performance plan R1–R4 |
| Comet-tail definition | unspecified | deferred to post-beta fast-follow (ruling R3c) |
| WP16 resume | deferred | remains deferred until Q13 hardware/credential prerequisites |

## 4. Overall evaluation

Engineering discipline is the project's strongest asset: the command-queue
architecture, retained-asset invariants, evidence-cited change log, and the
agent protocol have repeatedly caught real defects (WP14 Windows persistence,
WP9 reticle projection, Horizons route failure) before they compounded. Two
structural risks remain. First, everything left between here and Steam beta is
gated on physical hardware and credentials — no amount of further hosted CI
or M2 Pro work retires WP16/WP17; the purchasing decision is now the critical
path. Second, product-experience debt is concentrated in the four unruled
design questions; those are resolved in the companion plan so implementation
can proceed without another round-trip.
