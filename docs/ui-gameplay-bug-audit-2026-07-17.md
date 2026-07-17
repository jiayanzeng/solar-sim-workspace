# UI/gameplay implementation audit and corrective plan — 2026-07-17

## Status and authority

This began as a documentation-only audit of the current UI and gameplay
implementation. `ARCHITECTURE.md` remains the read-only design of record. The
human maintainer approved the corrective order and authorized Phase 1 on
2026-07-17. Work packages are reopened only at their documented phase boundary.

The review covered ARCHITECTURE §§3, 7–10, 12, and 13; `TASKS.md`; the completed
stabilization and conformance plans; the production source under
`crates/solar-sim/src`; the relevant `sim-core::time` boundary; and Bevy 0.19's
locally pinned input-focus, UI-picking, and render-error behavior. Steam/WP16,
WP17 hardware gates, catalog composition, generated assets, truth fixtures,
and physics/catalog accuracy are outside this cycle.

No source code was modified during this audit.

## Human authorization and Q17 ruling

On 2026-07-17 the human approved the complete corrective sequence: WP5
camera/input → WP8 toasts → WP14 epoch normalization and OOM recovery →
integrated closeout. Phase 1 may begin after this documentation baseline is
submitted.

Q17 is closed by human ruling. The OOM notification must use a native platform
surface exposed through `winit` or an equivalent platform abstraction already
present in the dependency tree. The notification must be invoked synchronously
or correctly marshalled to the main thread so it gains focus before shutdown
or a stopped-renderer state. A Bevy UI node and the window title are not
sufficient. `OutOfMemory → StopRendering` remains exact, and the ruling does
not authorize a new dependency or any `Cargo.toml` edit.

## Verification baseline

The audit started from a clean
`codex/ui-gameplay-remediation` branch synchronized with its remote. The
following read-only gates pass before any corrective work:

- `cargo test`: **337 passed** (53 `sim-core` · 233 `solar-sim` · 48 `xtask`
  library · 2 xtask smoke · 1 active spot-check), zero failures.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed with zero
  warnings.

These results show that the current automated suite is green; they do not
invalidate the interaction, layout, persistence, and recovery gaps below. Each
gap identifies the missing regression that allowed it to remain green.

## Executive disposition

| ID | Severity | Owner | Classification | Disposition |
|---|---:|---|---|---|
| UA-1 | high | WP5 | gameplay logic flaw | corrective work required |
| UA-2 | medium | WP5 | UI/gameplay input-ownership flaw | corrective work required |
| UA-3 | high | WP8 | direct ARCHITECTURE §9.6 non-compliance | corrective work required |
| UA-4 | medium | WP8 | responsive-interface flaw | corrective work required |
| UA-5 | medium | WP14 | settings/runtime contract mismatch | corrective work required |
| UA-6 | critical | WP14 | direct ARCHITECTURE §8.5 non-compliance | approved native-surface correction required |

The findings are grouped into work-package phases rather than isolated widget
patches. This preserves one-WP-at-a-time execution and lets shared ownership,
state, layout, and tests be corrected together.

## Detailed findings

### UA-1 — Dolly during travel causes a discontinuous zoom reversal

**Architecture and intended behavior.** ARCHITECTURE §§8.2 and 10.3 assign the
camera an eased travel tween to a moving focus with emergent follow. WP5 adds
an orbit rig with dolly and requires the travel tween to remain interruptible.
A user dolly during that tween must continue from the camera pose the user can
currently see.

**Implementation evidence.** In `control.rs`, the `Dolly` reducer immediately
multiplies `camera.distance_units`. If travel is active, it then replaces only
`travel.target_distance_units`. The subsequent travel update interpolates
again from the unchanged `travel.start_distance_units` to that new target at
the already-advanced eased progress.

For a deterministic example, let travel start at 100 units and target 20. At
half progress the visible distance is 60. A dolly factor of 0.9 makes the
visible distance 54 and changes the target to 54, but leaves the start at 100.
The next travel update at approximately the same progress evaluates the new
100→54 curve near 77, moving the camera outward immediately after the user
asked it to move inward.

**Impact.** Zoom can jump or reverse for one or more frames during Search,
Browse, label, or sphere travel. The final target may be valid, so existing
convergence and clamp tests do not expose the visible discontinuity.

**Missing coverage.** There is no transition test that applies dolly at
multiple in-flight tween progress values and asserts continuity plus monotonic
response from the pre-command visible pose.

### UA-2 — Ordinary HUD surfaces do not own raw right-drag or wheel input

**Architecture and intended behavior.** ARCHITECTURE §8.2 orders input and UI
before command reduction, while §9 defines controls that must not manipulate
the camera behind the user's interaction. The integrated stabilization design
also makes the raw-input system the sole owner of orbit/dolly generation and
requires UI ownership to suppress background gameplay.

**Implementation evidence.** `input_intent.rs::sync_pointer_capture` records
only whether the pointer is over a `UiScrollSurface`. In Gameplay context,
`collect_raw_intents` emits `Orbit` for every right-button mouse-motion event,
regardless of HUD hover. Wheel input is suppressed only for a scroll surface,
not for an ordinary top-bar, panel, rail, button, or other HUD surface.

**Impact.** Right-dragging over normal HUD can orbit the camera. Wheel movement
over a non-scroll HUD region can dolly the camera behind the interface. This is
an ownership gap, not a request to make the entire viewport a modal UI surface.

**Missing coverage.** Existing tests cover modal suppression and registered
scroll-surface wheel capture, but do not compare topmost HUD hover with
topmost viewport hover for right-drag and wheel.

### UA-3 — Toast nodes are pointer-blocking despite the non-blocking contract

**Architecture and intended behavior.** ARCHITECTURE §9.6 defines toasts as
"Non-blocking, bottom-left, auto-dismiss."

**Implementation evidence.** `time_bar.rs::toast_stack` and
`ui_kit::widgets::toast` create ordinary Bevy UI `Node`s without
`Pickable::IGNORE`. The pinned Bevy 0.19 UI picking backend explicitly treats
nodes without a `Pickable` override as picking participants that block targets
below them. The toast stack is at global z-index 105, above the ordinary HUD,
and occupies the bottom-left region.

**Impact.** A transient notice can intercept pointer input intended for the
left panel or another control beneath its rectangle. Visual transparency does
not make the node non-blocking.

**Missing coverage.** Current toast tests cover transition-only spawning and
auto-dismiss behavior, but do not perform a picking pass that proves the
underlying target remains the hit while a toast is visible.

### UA-4 — The fixed toast width overflows the required small/high-scale view

**Architecture and intended behavior.** The §9 UI must remain usable as a
product interface, and the existing stabilization acceptance matrix includes
800×600 through 4K at UI scales 0.75–2.0. A notice must remain inside that
usable viewport rather than depend on clipping.

**Implementation evidence.** `toast_stack` uses a fixed logical width of 390
pixels plus a 16-pixel left inset and has no viewport-relative width or maximum
constraint. At the required 800×600, 2.0-scale case, the scaled width plus
inset exceeds the physical viewport. The existing reachability matrix does not
instantiate an active toast stack.

**Impact.** Notice text and its surface can be clipped or extend beyond the
window precisely at the smallest accessibility-scale configuration.

**Missing coverage.** No resolved-layout test measures an active toast at each
required viewport/UI-scale pair or checks its physical bounds and text wrap.

### UA-5 — Persisted fixed epoch can disagree with the actual boot epoch

**Architecture and intended behavior.** ARCHITECTURE §7 limits the simulation
to 1800-01-01 through 2300-12-31 TDB. Section 8.5 persists the selected start
epoch. The setting displayed and saved to disk must therefore describe the
epoch used to initialize the simulation.

**Implementation evidence.** `AppSettings::normalized` replaces only a
non-finite fixed JD; it does not constrain a finite JD to the supported range.
The Settings `−1 YEAR` and `+1 YEAR` actions are also unbounded. In contrast,
`SimClock::new` clamps the converted epoch to `T_MIN_S..=T_MAX_S`. The Settings
screen continues to display and persist the original out-of-range JD.

An existing test demonstrates the split by passing an out-of-range fixed epoch
and asserting only that the resulting clock is pinned to `T_MAX_S`. It does
not require the setting itself to become the canonical 2300 edge.

**Impact.** A user can save a start epoch after 2300 or before 1800, relaunch
at a different clamped epoch, and still see the impossible value in Settings.
Because constructor clamping produces no `TickReport`, startup also provides no
range-clamp toast explaining the discrepancy.

**Missing coverage.** No persistence/relaunch test requires serialized,
displayed, and `SimClock::jd_tdb()` values to be bitwise or explicitly
tolerance-equivalent at both supported edges.

### UA-6 — The post-OOM Bevy error screen cannot be rendered

**Architecture and intended behavior.** ARCHITECTURE §8.5 requires
`OutOfMemory → StopRendering` with a user-facing error screen. WP14 repeats
that requirement.

**Implementation evidence.** `product_render_error_policy` records the stopped
state, changes the native window title, and returns
`RenderErrorPolicy::StopRendering`. A later Update system,
`sync_render_error_screen`, spawns the Bevy UI error node from that state. The
locally pinned Bevy 0.19 `RenderErrorPolicy` documentation states that
`StopRendering` keeps the app alive but stops further rendering, and its error
handler leaves the renderer in the stopped/error state. Consequently, a UI
node spawned after the stop has no future rendered frame in which to become
visible.

**Impact.** The native title may change, but the architecture-required error
screen cannot appear. The current state-machine unit test verifies the policy
directive and phase, not a displayed recovery surface.

**Human ruling.** Drawing another Bevy frame after returning `StopRendering`
conflicts with the selected safety policy, and the window title is not an
adequate critical-error surface. Q17 therefore requires a native platform
surface through `winit` or an equivalent platform abstraction already in the
dependency tree, invoked synchronously or marshalled to the main thread. No
new dependency is authorized.

## Reviewed behavior that is not a defect

- Successful Search selection intentionally commits the body name and restores
  live Search focus. The completed stabilization record requires this. Bevy's
  tab-navigation picking observer clears that focus when the user presses the
  viewport, so the earlier suspicion of an unreleasable text-input trap is not
  substantiated.
- Category layer switches intentionally hide their body spheres rather than
  coupling every label, orbit, and reticle to the same category switch. The
  independent Labels, Orbits, and Icons groups are architecture-defined.
- Saturn remains text-only. Its architecture-valid aggregate is sphere,
  rings, text label, and orbit; Io retains the representative reticle coverage
  required by the closed Q16 ruling.
- Hyperbolic orbital-period omission remains justified and deferred. An
  unbound hyperbolic body has no orbital period, and WP10 already permits the
  period field to be absent for that case.

## Ordered corrective plan

### Phase 1 — Repair camera and pointer ownership as one WP5 change

**Objectives.** Remove the in-flight dolly discontinuity and make raw camera
input respect explicit HUD-versus-viewport ownership without weakening modal,
text-edit, scroll, or replay behavior.

**Issues addressed.** UA-1 and UA-2.

**Pre-code documentation gate.** Re-read ARCHITECTURE §§3.4, 3.7, 8.2, 9,
10.3, and 12; the WP5 brief; the stabilization input-ownership design; and
this report. Record that review and the exact intended state transition in the
`TASKS.md` change log before editing production code. Reopen only WP5.

**Implementation steps.**

1. Rebase an active travel's distance interpolation from the currently visible
   distance when a dolly command arrives. Preserve the moving-focus path,
   elapsed/duration semantics, framing clamps, deterministic f64 state, and
   final follow behavior.
2. Define explicit pointer ownership for ordinary HUD surfaces rather than
   treating every UI node as HUD; the full-window viewport pick surface must
   remain gameplay-owned.
3. Have the sole raw-input collector suppress right-drag orbit and wheel dolly
   when the topmost hit is HUD-owned. Registered scroll surfaces continue to
   consume their own wheel input, and modal/text contexts continue to suppress
   all gameplay input.
4. Keep emitted actions on the existing `InputIntent → SimCommand` path. Do
   not mutate camera state from UI or picking observers.
5. Add transition and integration regressions before changing completion
   status.

**Acceptance criteria.**

- Dolly at the start, 25%, 50%, 75%, and near-completion of a travel changes
  distance in the requested direction without a next-frame reversal or jump.
- The rebased tween remains continuous, respects zoom limits, reaches its
  deterministic final target, follows a moving body, and is still interruptible
  by a new selection.
- Right-drag and wheel over representative top-bar, left-panel, right-rail,
  Layers-panel, and Settings controls enqueue no Orbit/Dolly commands.
- Right-drag and wheel over the viewport still enqueue the existing commands;
  scroll surfaces scroll without camera dolly.
- Desktop and headless reducers, replay parsing/hash behavior, axis inversion,
  Search/Browse/Settings ownership, picking, and keyboard mappings remain
  unchanged except for the intended pointer capture.
- Targeted tests plus every repository submission gate pass, and the workspace
  test count does not decrease.

**Submission standard.** Once all criteria are confirmed, update WP5 status
and add the evidence to the newest `TASKS.md` change-log entry. Stage only the
phase files, inspect the staged diff, commit the code/tests/docs, and push the
current `codex/` branch automatically. Do not submit a partial phase.

### Phase 2 — Make toasts non-blocking and responsive as one WP8 change

**Objectives.** Implement the literal non-blocking toast contract and keep
active notices inside every supported viewport/UI-scale combination.

**Issues addressed.** UA-3 and UA-4.

**Pre-code documentation gate.** Re-read ARCHITECTURE §§7, 8.4, 9.6, and 12;
the WP8 brief; prior reachability rules; and this report. Record the reviewed
pointer and geometry contracts in `TASKS.md`, then reopen only WP8.

**Implementation steps.**

1. Mark the complete toast picking subtree as pass-through using the pinned
   Bevy 0.19 picking contract, including text descendants that could otherwise
   become topmost hits.
2. Replace the unconditional fixed-width geometry with a bounded layout that
   preserves the intended desktop width when space exists and wraps within the
   available small/high-scale viewport.
3. Preserve bottom-left placement, z-order visibility, transition-only
   creation, accessible notice semantics, and delayed auto-dismiss.
4. Add actual picking and resolved-layout regressions with an active toast.

**Acceptance criteria.**

- With a toast visible, an underlying test target receives the same pointer hit
  as it does without the toast; neither the toast root nor a descendant owns
  the pointer.
- At every required viewport and UI scale, the resolved toast bounds remain
  inside the physical viewport, text wraps without clipping, and the time bar
  remains unobstructed.
- Multiple notices retain bottom-left stacking and spacing, appear only on the
  documented `TickReport` transitions, and auto-dismiss exactly once.
- Accessibility labels remain present without making the toast pointer-active.
- Targeted tests plus every repository submission gate pass, and the workspace
  test count does not decrease.

**Submission standard.** Once all criteria are confirmed, update WP8 and its
evidence, inspect and stage only the phase diff, commit, and push automatically.

### Phase 3 — Normalize the epoch and implement native OOM recovery under WP14

**Objectives.** Make the Settings value, serialized value, and boot clock agree
through one reviewed normalization boundary, and satisfy both halves of the
exact §8.5 OOM contract with the human-approved native platform surface.

**Issues addressed.** UA-5 and UA-6.

**Pre-code documentation gate.** Re-read ARCHITECTURE §§3.5, 7, 8.2, 8.5, and
12; the WP14 brief; Q15 reset/persistence rules; the closed Q17 ruling; the
pinned Bevy/winit recovery APIs; and this report. Record the exact TDB/JD
boundary and native main-thread invocation mechanism in `TASKS.md`, then reopen
only WP14. The ruling does not authorize a new dependency or `Cargo.toml` edit.

**Implementation steps.**

1. Derive the legal fixed-JD interval from the existing public
   `sim-core::time` range and conversion functions; do not duplicate calendar
   constants or perform a UTC conversion.
2. Normalize finite out-of-range settings at the same committed settings
   boundary used by Apply, startup loading, and reset. Persist the canonical
   fixed JD so the next launch displays what the clock uses.
3. Clamp or disable the Settings year-step actions at the same edges and avoid
   generating a false dirty state when an outward step cannot change the value.
4. Remove the unreachable post-stop Bevy-screen path or restrict it to states
   where rendering can still occur.
5. Invoke the approved native error surface synchronously at the OOM failure
   boundary, or marshal it to the platform main thread with an ordering proof
   that it gains focus before shutdown or the renderer remains stopped.
6. Preserve `DeviceLost → Recover`, unexpected-error behavior, LIVE mode,
   reset defaults, settings-file corruption fallback, command routing,
   deterministic simulation state, and exact layer persistence.
7. Add corrupt/out-of-range loader and isolated process-relaunch coverage plus
   the highest-feasible native-surface boundary test. A state-enum-only OOM
   test is insufficient; evidence must show that the surface invocation occurs.

**Acceptance criteria.**

- Finite fixed JDs below/above the supported range canonicalize to the exact
  core minimum/maximum; NaN and infinities retain the documented safe default.
- After Apply and after process relaunch, the draft, committed setting,
  serialized TOML, displayed `FIXED JD`, and `SimClock::jd_tdb()` agree at both
  edges.
- Repeated outward year steps at an edge do not move beyond it or produce
  unnecessary save traffic; an inward step works immediately.
- An injected OOM invokes the native platform surface exactly once on the
  approved thread/path, returns `StopRendering`, and does not rely on a later
  Bevy-rendered frame or the window title as the critical notification.
- The OOM message is prominent and actionable; repeated callbacks do not
  duplicate the surface or panic.
- Device loss still requests renderer recreation and clears recovery state
  only after the device is restored.
- LIVE, `RESTORE DEFAULTS`, `--reset-settings`, UI/layer persistence, the
  non-persisted visual-cue floor, normal startup, headless tests, and macOS and
  Windows compilation paths retain their existing behavior.
- Targeted tests plus every repository submission gate pass, and the workspace
  test count does not decrease.

**Submission standard.** Once all criteria are confirmed, record the WP14 and
Q17 implementation evidence, inspect and stage only the phase diff, commit,
and push automatically.

### Phase 4 — Integrated closeout

**Objective.** Verify that the accepted phases compose correctly and update
the authoritative status record without broadening scope.

**Implementation steps.**

1. Re-read this complete report, every phase change-log entry, and the final
   source diff; cross-check the six findings rather than testing modules in
   isolation.
2. Run all targeted interaction/layout/relaunch/recovery tests and the full
   repository gates below.
3. Confirm that Saturn/Io behavior, hyperbolic period handling, Q15 recovery,
   replay compatibility, physics results, and deferred Steam work did not
   change.
4. Update `TASKS.md` with final evidence and the new nondecreasing test count.

**Acceptance criteria.**

- UA-1 through UA-6 are closed by source and regression evidence, including
  the native-surface invocation boundary required by the closed Q17 ruling.
- `cargo test`, `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and
  `git diff --check` pass from the final tree.
- No test was weakened or removed without a numerical/behavioral justification;
  no test count decreased.
- No `ARCHITECTURE.md`, `AGENTS.md`, generated catalog, spot-check truth,
  dependency, catalog-composition, physics-tolerance, Steam/WP16, or WP17
  hardware change entered the cycle without its separate human authorization.

**Submission standard.** If closeout changes documentation only, commit and
push that evidence automatically after all applicable criteria pass. Never
describe UA-6 as complete without its native-surface invocation evidence.

## Repository-wide submission gates for every source phase

Each phase is atomic. Before every automatic GitHub submission:

1. The cited architecture, work-package brief, this report, and prior phase
   documentation have been reviewed and the pre-code review is recorded.
2. Exactly one coordinating WP is `in-progress`; unrelated changes are absent.
3. New behavior and the missing regression land together under ARCHITECTURE
   §12.
4. `cargo test` is green and the workspace test count has not decreased.
5. `cargo fmt --all -- --check` is green.
6. `cargo clippy --workspace --all-targets -- -D warnings` is green.
7. `git diff --check` is green and the staged diff contains only the accepted
   phase.
8. `TASKS.md` records status and exact evidence before the commit.
9. No read-only file, generated/truth asset, dependency, Steam work, or
   unrelated refactor is included.

Failure of any criterion stops submission. An ambiguity becomes an Open
question; it is not resolved by weakening a test or improvising a new design.
