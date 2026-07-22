# Codex task — UIP-9: primary-surface availability gate (implements ruling D5)

**Self-contained.** Everything you need is in this block plus the repo. Human
ruling D5 is supplied verbatim below; you are **transcribing** it, not
authoring it. Do not reword any quoted ruling text.

---

## 0. Read first, in this order

1. `AGENTS.md` (root) — hard rules, definition of done.
2. `ARCHITECTURE.md` — read-only. Do not edit.
3. `TASKS.md` — update protocol, `Q18 — OPEN`, change log.
4. `docs/decision-record-2026-07-22.md` — D3 (the ruling D5 amends).
5. `docs/ui-performance-plan-2026-07-22.md` — the UIP-8 block and its
   2026-07-22 execution note.
6. `docs/wp15-golden-screenshots.md` — §Window-surface smoke check.
7. `crates/solar-sim/src/lib.rs` — `SmokeFrames`, `smoke_readiness_gate`,
   `SMOKE_READINESS_TIMEOUT_S`, `SMOKE_READINESS_TIMEOUT_ERROR`,
   `assert_window_nonblack_and_exit`, `nonblack_rgb_dimensions`.
8. `crates/solar-sim/src/golden.rs` — `ReferencedTextureInputs::
   ready_for_initial_readback`, `readback_settle_complete`,
   `MIN_SETTLE_FRAMES`, `MIN_SETTLE_SECONDS`.

## 1. Why this exists (the finding behind D5)

Two facts make the current gate structurally unable to close Q18:

1. **The approved readiness condition is operationally a five-second sleep.**
   `MIN_SETTLE_SECONDS = 5.0` and every recorded wait — 5.000, 5.001, 5.008 —
   is that floor binding. The assets-only probe already proved textures report
   ready at 0.000 s. Passing and failing runs are indistinguishable in the
   printed duration, so the "detectable signal" UIP-8 was told to emit is
   measuring the non-binding variable.
2. **The condition was imported from a path that never touches the swapchain.**
   Golden capture reads back a dedicated offscreen `Image`
   (`GoldenRenderTarget`, 960×600). `--assert-nonblack` reads back
   `Screenshot::primary_window()` — the real surface. Golden success is
   therefore structurally incapable of being evidence about window-surface
   readiness. D3's root-cause premise was correlation across two different
   capture paths.

D5 supplies the missing conjunct. It is a correction to D3, not a workaround.

---

## 2. Ruling D5 — transcribe verbatim into `docs/decision-record-2026-07-22.md`

Append after D4, under a `## D5 — Q18 amended: primary-surface availability
conjunct` heading, with the date 2026-07-22 and a note that it amends D3.

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

D5 also authorizes the new plan block **UIP-9** described in §3.

---

## 3. Transcribe UIP-9 into `docs/ui-performance-plan-2026-07-22.md`

Add after the UIP-8 block. Do not delete or reword UIP-8 or its execution
note; UIP-8 remains a completed implementation of a superseded condition.

> ### UIP-9 — Primary-surface availability conjunct (implements ruling D5)
>
> **Build.** Compose the existing shared initial-readback settle condition with
> a new `--assert-nonblack`-local primary-surface availability predicate;
> readiness is the conjunction. Report the two conjuncts' satisfaction times
> separately so the printed signal measures the binding variable. Preserve the
> single 10 s deadline, the one-shot readback, and the unchanged assertion.
>
> **Out of scope.** `golden.rs` behavior, the shared condition's signature and
> semantics, renderer defaults, the WP17 command text, retry of any kind.
>
> **Acceptance.** As stated in D5: ten consecutive passes across two sessions,
> a hardware negative control, and the unit regressions. Q18 remains open until
> the human closes it on that evidence.

---

## 4. Scope

**In scope**

- `crates/solar-sim/src/lib.rs` — the `--assert-nonblack` readiness path only.
- `TASKS.md`, `docs/decision-record-2026-07-22.md`,
  `docs/ui-performance-plan-2026-07-22.md`, `docs/wp15-golden-screenshots.md`,
  and `README.md` **only if** it states the retired five-pass bar.

**Out of scope / forbidden**

- Any behavior change in `crates/solar-sim/src/golden.rs`. A `//!` doc line
  noting that the shared condition is composed, not replaced, is the only
  permitted edit there. The shared function signature and semantics of
  `ready_for_initial_readback` / `readback_settle_complete` do not change.
- Any retry, any added delay, any weakening of `nonblack_rgb_dimensions` or
  its exact string `"primary window readback is entirely black"`.
- Renderer defaults, quality presets, window mode, scale factor.
- New dependencies or any `Cargo.toml` edit.
- Editing `ARCHITECTURE.md`, any `AGENTS.md`, `assets/catalog*.ron`, or
  anything under `xtask/fixtures/spotcheck/`.
- Closing Q18. Humans close open questions.
- Changing the WP17 command text.

---

## 5. Task order

### 5.1 Record the phase start

Before touching source, add a `TASKS.md` change-log entry opening UIP-9 as the
sole in-progress phase, citing D5, stating the scope above, and recording the
pre-change green baseline from `cargo test` (expected 402 default / 403 with
`--features steam`; record whatever you actually observe).

### 5.2 Transcribe D5 and UIP-9

Per §2 and §3.

### 5.3 Source change

Add to `crates/solar-sim/src/lib.rs`:

```rust
const SMOKE_SURFACE_READY_FRAMES: u32 = 30;
const SMOKE_SURFACE_TIMEOUT_ERROR: &str =
    "the primary window surface was not continuously available for 30 frames \
     within 10 seconds after the smoke frame-count measurement; the window must \
     be foreground and unoccluded";
```

Extend `smoke_readiness_gate` to a pure three-argument function. Asset/settle
precedence on a double failure is deliberate — it is the earlier stage, so a
run failing both reports the more fundamental cause — and it is tested:

```rust
fn smoke_readiness_gate(
    all_loaded: Result<bool, String>,
    surface_available_frames: u32,
    elapsed_s: f64,
) -> Result<bool, String> {
    let assets_ready = match all_loaded {
        Err(error) => return Err(format!("referenced-texture readiness failed: {error}")),
        Ok(ready) => ready,
    };
    let surface_ready = surface_available_frames >= SMOKE_SURFACE_READY_FRAMES;
    if assets_ready && surface_ready {
        return Ok(true);
    }
    if elapsed_s >= SMOKE_READINESS_TIMEOUT_S {
        return Err(if !assets_ready {
            SMOKE_READINESS_TIMEOUT_ERROR.into()
        } else {
            SMOKE_SURFACE_TIMEOUT_ERROR.into()
        });
    }
    Ok(false)
}
```

Track the signal on `SmokeFrames` (or an adjacent resource if that reads
better): a consecutive-frame counter that **resets to zero** on any observed
unavailability, counted only over the readiness window (mirroring the existing
`smoke.seen.saturating_sub(target)` pattern), plus the last observed raw state
for diagnostics.

**Signal tier — implement (1), fall back to (2), report which landed.**

- **Tier 1 (preferred, a proof).** A render-world observation that the primary
  window's swapchain texture was successfully acquired this frame, made
  readable from the main world through a mechanism already available in the
  dependency tree — e.g. an atomic shared as a resource in both worlds,
  incremented by a render-world system and read by the main-world gate, with
  "increased since last main-world frame" meaning acquired. Verify against the
  actual Bevy 0.19 API surface; do not assume names from memory.
- **Tier 2 (a proxy, only if tier 1 is unreachable without a new dependency or
  render-app internals).** Main-world window occlusion state and focus:
  available means not occluded **and** focused **and** not minimized. If you
  land tier 2 you must additionally document, in the module comment and in the
  `TASKS.md` entry, the **initial-state assumption** — on macOS the occlusion
  change event may not fire for a window that comes up already occluded, so
  the pre-first-event assumption is a named residual risk, not a proof.

**Diagnostics.** Print, at readback-request time and therefore regardless of
outcome:

- the elapsed time at which the settle conjunct first became true;
- the elapsed time at which the surface conjunct first became true;
- the consecutive available-frame count and the last observed raw state;
- the tier in use.

This is the point of the change: the printed signal must finally measure the
binding variable rather than re-reporting the 5-second floor.

**No added delay.** The surface conjunct normally accumulates its 30 frames
during the existing 5-second settle window, so the ordinary path gains zero
wall-clock. Do not introduce a separate deadline; the single existing 10 s
budget covers both conjuncts.

### 5.4 Tests required

Land with the code, in the same change:

1. Extend `smoke_readiness_timeout_fails_loudly_before_any_readback_request`
   for the new argument, preserving every existing assertion.
2. New: assets ready, surface starved → `SMOKE_SURFACE_TIMEOUT_ERROR` at the
   deadline, `Ok(false)` before it.
3. New: **both** conjuncts unsatisfied at the deadline →
   `SMOKE_READINESS_TIMEOUT_ERROR` (precedence is asserted, not incidental).
4. New: surface conjunct at exactly `SMOKE_SURFACE_READY_FRAMES - 1` and at
   `SMOKE_SURFACE_READY_FRAMES` — boundary is exact.
5. New: the consecutive counter resets on an unavailable observation and does
   not accumulate across a gap.
6. Unchanged: a ready frame remains exactly one readback request, never a
   retry or a timeout.

Test names are sentences, per house style. **The unit tests are not the
negative control** — they prove the string and the path; §5.6 proves the
signal observes real occlusion.

### 5.5 Docs

- `docs/wp15-golden-screenshots.md` §Window-surface smoke check: replace the
  paragraph describing the texture-only readiness condition and the paragraph
  recording the unrepeatable UIP-8 evidence. State the conjunction, the three
  distinct failure messages, the operator precondition (foreground, unoccluded
  — now enforced: the gate refuses rather than captures), and that Q18 remains
  open pending the D5 acceptance evidence.
- `README.md`: correct only if it states the retired five-pass bar. Q18 stays
  listed as open.

### 5.6 Hardware acceptance (M2 Pro / Metal)

Exact command, unchanged:

```sh
target/release/solar-sim --smoke 60 --expect-backend metal \
  --reject-software-adapter --assert-nonblack
```

1. **Ten consecutive passes across two sessions.** A session boundary is a
   logout/login or reboot — not a new terminal. If you cannot force a session
   boundary, stop after the first session, record what you have, and hand back
   stating exactly what you need the human to run.
2. **Negative control.** One run in which the window is deliberately occluded
   during the readiness window. It must exit nonzero with
   `SMOKE_SURFACE_TIMEOUT_ERROR`, must **not** reach a readback, and must not
   fail with the black-readback string. Record the exact occlusion method used.
3. Record, for every run: pass/fail, both conjunct satisfaction times, the
   consecutive frame count, the tier, and the readback dimensions.

### 5.7 Verification gates

`cargo test` (default and `--features steam`), `cargo fmt --all -- --check`,
warning-denied workspace/all-target clippy for both feature sets, release build
plus release warning-denied library clippy, `git diff --check`, texture metadata
audit, catalog dry-run. The workspace test count may only go up; state the new
count with its crate split.

### 5.8 Close-out

Append a `TASKS.md` change-log entry with all evidence, and update the `Q18 —
OPEN` section to record D5, the tier that landed, the new acceptance bar, and
the evidence status. **Leave Q18 open.** The human closes it.

---

## 6. Stop and file a question instead of improvising if

- Tier 1 would require a new dependency, a `Cargo.toml` edit, or editing a
  read-only file.
- Neither tier is implementable against the actual Bevy 0.19 API surface.
- You cannot force a session boundary for the second acceptance session.
- **A run returns black with surface availability affirmatively observed across
  30 consecutive frames.** Stop immediately. Do not iterate, do not add a
  delay, do not retry. Record it under Q18 as escalation evidence per D5 and
  hand back.

## 7. Report back

Dense prose, answering in order: which tier landed and why; the exact new
constants and message strings; the test names added and the new counts; every
hardware run with its recorded fields; the negative-control method and result;
which files changed; anything you stopped on.
