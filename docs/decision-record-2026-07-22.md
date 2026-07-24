# Decision record (2026-07-22) — delegated closures and Windows plan

**Authority.** The human authorized proceeding on non-UI/perf issues per
Claude's recommended approach without further consultation (chat,
2026-07-22). Closures below should be transcribed into `TASKS.md` by the next
agent session under the normal protocol, citing this record as the human-
delegated authorization. Rulings R1–R4 for the UI queue live in the companion
UI/performance plan.

## D1 — Q4 closed: in-house constellation line set

Adopt option 4 of the 2026-07-12 brief: an in-house authored line set of
HR-number pairs over the already-licensed HEASARC BSC5P starfield source.
Zero license risk, matches ARCHITECTURE §10.5's "license-clean line set"
language, and reuses the WP13 asset pipeline. Scope: post-beta fast-follow;
first tranche is the ~20 most recognizable figures, authored as data for
human review before any render work. GPL (Stellarium) and chart-artwork
(S&T/IAU) sources remain prohibited; d3-celestial is rejected to avoid
per-file provenance archaeology.

## D2 — Q12 closed: superseded

The CI restructure that CI-1…CI-6 were meant to brief has shipped and is the
operating reality: `ci.yml` (lint / test-linux / macos-14 + windows-latest
platform matrix with Metal hard gate and WARP `continue-on-error` smoke /
invariants with MSRV, core purity, offline rule, Steamworks guard) plus the
`workflow_dispatch`-only `goldens.yml` with the macOS/Metal-blocking
perceptual gate. The CI-1…CI-6 numbering is retired; no agent may resurrect
it. Future CI changes get fresh, individually briefed Open questions.

## D3 — Q18 ruled: readiness precondition, hard assertion retained

Root cause assessment: the one-shot `Screenshot::primary_window()` readback
races surface readiness; the golden harness's referenced-texture wait
demonstrably produces nonblack frames on the same machine seconds apart, and
2026-07-22 evidence proved frame count alone is not a readiness condition.
Ruling: the gate procedure gains an explicit readiness precondition — the
readback is requested only after the golden-harness readiness condition
holds, with a 10 s timeout that fails loudly (distinct message, nonzero
exit). No retries; the nonblack assertion is unchanged and remains one-shot.
The WP17 gate text is read as "60 measured frames, then readiness, then one
readback." Implementation is block **UIP-8** in the companion plan; Q18
closes when that block's five-consecutive-pass evidence is recorded.

## D4 — Q13 hardware half: purchase both reference machines

**Decision: acquire the WP17 reference hardware as written rather than amend
the brief.** Amending the reference machines requires a signed architecture
change and weakens the floor the perf gate exists to defend; used hardware
matching the brief is cheap and settles WP16 *and* WP17 evidence in one
purchase cycle.

1. **Windows / GTX 1650-class laptop** (satisfies the WP17 perf gate
   verbatim, plus DX12 goldens, SteamPipe dev-branch launch, the Windows
   overlay spike, and the `--expect-backend dx12 --reject-software-adapter`
   real-hardware smoke). Target: used/refurbished Acer Nitro 5, Lenovo
   IdeaPad Gaming 3, or Dell G3-class unit with a **GTX 1650 (not Max-Q-only
   if avoidable), 16 GB RAM (buy 8 GB only if trivially upgradable),
   1920×1080 display, NVMe SSD, Windows 11 Home**. Typical used price
   US$300–450. On receipt: clean Windows install, current NVIDIA driver
   (record version), disable OEM overlays, install Steam.
2. **M1 MacBook Air** (base 7-core GPU / 8 GB is the honest floor — do not
   "upgrade" the reference). Used market US$450–600. This machine is the
   binding Metal perf gate; the M2 Pro numbers do not substitute.

The credential half of Q13 (partner account, real App ID, Apple Developer ID,
protected environments) remains open and human-owned; nothing here touches
it.

## D5 — Q18 amended: primary-surface availability conjunct

**Date:** 2026-07-22. **Amends:** D3.

> **D5 — Q18 amended (supersedes D3's readiness definition for
> `--assert-nonblack` only)**
>
> Scope: the `--assert-nonblack` path only. Golden capture is unchanged.
>
> Readiness becomes a conjunction: the existing settle condition (≥30 frames,
> ≥5 s, materials present, base-color textures loaded) **AND** an explicit
> primary-surface availability signal holding for ≥30 consecutive frames
> immediately preceding the readback.
>
> Signal, in preference order: (1) a render-world signal that the primary
> window's swapchain texture was successfully acquired, plumbed to the main
> world through an existing Bevy mechanism; (2) if Bevy 0.19 does not expose
> (1) without reaching into render-app internals, a main-world proxy
> (occlusion state, focus, non-minimized). No new dependency, no `Cargo.toml`
> edit. The change log must state which tier landed, and a tier-2
> implementation must be labelled a proxy in the module header and in
> `TASKS.md` — not as proof.
>
> Unchanged from D3: one 10-second deadline total (not a second budget),
> one-shot readback, no retry, no added delay, assertion unchanged. The
> surface-unavailable timeout exits nonzero with a message distinct from both
> the texture-failure and settle-timeout strings.
>
> On a black readback, the last observed surface state is printed so the frame
> is attributable after the fact.
>
> A foreground, unoccluded window becomes a documented operator precondition
> in the WP17 procedure — now enforced by the gate rather than promised by the
> operator.
>
> **Acceptance bar replaced.** D3's "≥5 consecutive passes" has already
> produced a false green: the five-pass streak was followed by two failures on
> the same binary. Q18 closes only on (a) ≥10 consecutive passes across ≥2
> separate sessions separated by a logout/login or reboot, each printing
> affirmative surface availability; (b) a hardware negative control in which a
> deliberately occluded window fails with the surface-unavailable message
> rather than with a black readback; (c) the unit regressions below.
>
> **Escalation, pre-authorized to reporting only.** If a run still returns
> black with surface availability affirmatively observed across 30 consecutive
> frames, the occlusion hypothesis is dead and this is a Bevy 0.19 screenshot
> defect. The agent stops, records the evidence under Q18, and does not
> iterate. Acting on the escalation (upstream report, or a WP17 contract
> amendment redefining the window check as a composite) requires a further
> human ruling.

## D9 — Final-stage on-site Windows test plan
   (formerly the second D5; hardware TBD accepted)

Since on-site Windows testing is scheduled for the final stage, the plan is
written to be executable in one day on the D4 machine, in this order, with
each step's output pasted into TASKS.md:

1. **Environment record** — `dxdiag` summary, driver version, resolution,
   power plan (High performance, plugged in).
2. **Real-GPU smoke** — `cargo run -p solar-sim --release -- --smoke 60
   --expect-backend dx12 --reject-software-adapter --assert-nonblack`
   (post-UIP-8 build). This is the WP17 DX12 smoke gate.
3. **DX12 goldens** — `xtask capture-goldens` twice, `compare-goldens`
   against the approved Metal-anchored baselines under the perceptual
   thresholds; file deltas as evidence, not silent re-baselines.
4. **Perf gate** — UIP-1 `--frame-stats` runs, all layers on, at the six
   canonical views × {Medium, High}; record the table. Decision rule 1 from
   the perf plan governs the shipped default preset.
5. **Replay determinism** — run the replay library; assert state-hash parity
   with the macOS hashes.
6. **Settings persistence** — the full-process relaunch proof on real
   Windows (the WP14 suite already covers this in CI; re-run locally once).
7. **Steam overlay spike** — real client, interim App ID 480, per
   `docs/wp16-steam-overlay-spike.md`; document result either way.
8. **SteamPipe dev-branch install + launch** — requires the real App ID and
   partner account (credential half of Q13); if credentials are not yet
   provisioned, record steps 1–7 and defer 8 explicitly rather than faking
   it with 480, which packaging preflight will correctly refuse.

Contingency: if any of 2–4 fails, stop, file the evidence, and fix on the
Mac-side toolchain before a second on-site day — do not debug live on the
reference machine beyond capturing artifacts.

## D6 — License/public-repo posture (recorded recommendation)

A proprietary all-rights-reserved `LICENSE` now exists at the root (the
earlier gap is resolved). Standing tension to acknowledge: the repository is
public (for free hosted runners), so the commercial product's source is
world-readable and its published history is permanently public regardless of
any later privatization. Recommendation: accept **source-visible
proprietary** status through beta — it costs nothing now and several
commercial titles ship this way — and make an explicit go/no-go on
privatizing (with paid macOS CI minutes budgeted) at the 1.0 release
decision, alongside the already-planned protected-environment handling for
Steam/Apple secrets. No action required today.

## D7 — Q19–Q21 performance-maintenance rulings

**Date:** 2026-07-23. **Authority and full rationale:**
`docs/playability-review-and-rulings-2026-07-23.md` §2.

- Q19: Ultra retains an 8× request, but runtime application resolves the
  highest adapter-supported sample count at or below the request before it
  reaches the camera. Settings and frame statistics disclose requested and
  effective counts; an annotated Ultra-effective measurement may also satisfy
  High when both resolve identically.
- Q20: the two diagnostics-overlay tests are compiled only under
  `cfg(debug_assertions)`, allowing the additional release all-target clippy
  probe to compile without turning it into a required CI gate.
- Q21: the Retina toggle is declared windowed-effective. Borderless
  fullscreen renders at the display's physical resolution; the Settings copy
  and documentation must say so. An internal render-scale chain remains
  deferred unless reference-hardware evidence makes resolution the binding
  term.

## D8 — Q22–Q26 playability rulings and reveal interactions

**Date:** 2026-07-23. **Authority and complete acceptance detail:**
`docs/playability-review-and-rulings-2026-07-23.md` §§3–4.

- Q22: approve one replayable `ResetInterface` command restoring the
  launch-time session snapshot without writing settings or adding duplicate
  controls to every modal.
- Q23: approve fixed Menu inventories; column one expands the 26 planet
  moons, column two the six dwarf-planet moons, and the comet footer is inert.
- Q24: approve catalog-backed per-body orbit colors with corrected Venus,
  Jupiter, Saturn, and Neptune values plus exact uniqueness and CIE76 gates.
- Q25: approve category-specific overview floors, floor-before-exaggeration,
  and resolved Ceres/Pluto (optionally Charon) mission textures with honest
  representative albedo elsewhere.
- Q26: approve original 150–220 word NASA/JPL-cross-checked descriptions,
  validated Wikipedia links, and a dependency-free command-routed platform
  opener behind `PlatformServices`.
- R-NAV: travel to a body whose category is hidden first queues an explicit
  category-layer enable command.
- R-PRESET: Belt travel first enables Asteroids when necessary; the other
  region presets retain their existing framing-only semantics.

The exact Revision E text remains in the authority document §4 and is
human-maintainer work. Wave 0 may proceed before that edit; Wave 1 may not.
