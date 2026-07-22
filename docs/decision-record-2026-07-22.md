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

## D5 — Final-stage on-site Windows test plan (hardware TBD accepted)

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
