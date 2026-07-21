# UI and gameplay stabilization plan — 2026-07-16

## Purpose and authority

This document is the implementation brief for the post-WP15 UI/gameplay
stabilization cycle requested by the human maintainer on 2026-07-16. It does
not revise `ARCHITECTURE.md`; the architecture remains the design of record.
The work repairs interactions that do not yet satisfy Rev C's single-command
path, deterministic replay, modal-input, navigation, and high-rate rendering
contracts.

The implementation is deliberately integrated. Input ownership, commands,
replay, modal UI, navigation, persisted recovery, and render emphasis are
treated as one interaction system rather than independent widget patches.
No new dependency is required.

## Q15 human ruling

Q15 is closed with both of these requirements:

1. **Explicit reset recovery.** The application provides an in-product reset
   action and a `--reset-settings` launch path. Both restore reviewed defaults
   through the same semantic command/settings boundary; users never need to
   find or edit `settings.toml`.
2. **Minimum startup visual-cue floor.** Exact persisted layer values remain
   intact. If the saved startup view has the User Interface enabled but all
   three principal astronomical cues (`Orbits`, `Labels`, and `Icons`) disabled,
   the app shows a non-persisted, always-discoverable recovery notice with a
   one-action restore control. This safety surface does not silently enable a
   layer, does not alter the settings file, and disappears as soon as any cue
   is restored. If the User Interface itself is disabled, the existing
   `SHOW UI` affordance remains the sole recovery surface.

This interpretation provides the requested compatibility improvement without
weakening WP14's exact settings persistence: stored values round-trip exactly,
while a transient safety affordance prevents a visually empty startup from
becoming a dead end.

## Interaction architecture

### One semantic command path

Every user action that changes application-visible state crosses
`SimCommandQueue`. UI observers may maintain only transient edit mechanics
such as an `EditableText` buffer, pointer hover, or a pending scroll gesture.
The following state becomes command-owned:

- simulation clock and camera;
- global layers and fullscreen;
- body-size exaggeration, moon visibility, and local-orbit visibility;
- left-panel page and collapsed state;
- browse open/close and expanded columns;
- settings open/close, committed reset, and committed settings application;
- breadcrumb/navigation transitions.

Desktop and headless execution call the same reducers. Settings disk writes
occur only after a committed reducer transition. Replay v1 remains parseable;
new commands and frame inputs use an additive replay-v2 format.

### Interaction context

One resource describes the active input context:

- `Gameplay`
- `TextEdit`
- `BrowseModal`
- `SettingsModal`

The raw-input system is the sole owner of global keyboard/mouse state. It
suppresses gameplay hotkeys, orbit, dolly, and viewport picking whenever a
text editor or modal owns input. Focused text observers continue to handle
Enter/Escape locally, but their keypresses cannot leak into gameplay.
Escape resolves the topmost active context exactly once.

Pointer scroll on a registered scroll surface updates that surface and stops
camera dolly. Pointer scroll over the viewport continues to emit `Dolly`.

### Replay inputs

Replay v2 records the explicit per-frame inputs required by deterministic
desktop behavior:

- wall delta seconds;
- wall-clock TDB seconds used by `SimClock::tick` and LIVE state;
- ordered semantic commands for the frame.

Replays never read the system clock. A v2 recording that includes a LIVE snap
must reproduce the same clock, camera, layer, view-option, navigation, and
modal state hashes. Replay v1 remains supported with its caller-supplied fixed
delta and synthetic wall time.

## Ordered implementation tasks

### Task 1 — Command boundary and replay schema

**Objective.** Move every application-visible user transition onto
`SimCommandQueue`, add shared reducers, and introduce explicit replay-v2 frame
inputs.

**Acceptance.**

- View Options observers enqueue commands and never mutate
  `ViewOptionsState` directly.
- Browse, left-panel navigation, and committed settings/reset transitions are
  command-routed.
- Desktop and headless runners use the same reducers.
- Replay v1 still parses and executes.
- Replay v2 rejects unordered frames, non-finite deltas/wall times, timestamp
  mismatches, and corrupt commands.
- A variable-delta replay containing View Options, layers, +100 yr/s, and
  Snap-to-LIVE produces the same final combined state hash as the recording.

### Task 2 — Input ownership and modal behavior

**Objective.** Prevent text and modal interactions from generating background
gameplay actions.

**Acceptance.**

- Typing `s`, `m`, `i`, `o`, `r`, `p`, Space, `1`, `[` or `]` into search
  produces no gameplay command.
- Editing date/time produces no travel, rate, playback, orbit, or dolly
  command except the intended committed `SetTime`.
- Settings and Browse suppress gameplay keyboard, right-drag orbit, wheel
  dolly, label activation, and viewport picking.
- Browse uses modal tab navigation; focus cannot escape behind it.
- Escape reverts an active text edit, otherwise closes Browse, otherwise
  closes Settings, with exactly one transition.

### Task 3 — Scroll, focus continuity, and responsive reachability

**Objective.** Make long panels genuinely scrollable and preserve interaction
continuity across state updates.

**Acceptance.**

- Left-panel content and all three Browse columns own `ScrollPosition` and
  pointer-scroll handlers with clamped line/pixel motion.
- Scrolling these surfaces emits no `Dolly`.
- Settings retains its scroll position after changing any draft control.
- Settings, layers, and left-panel actions preserve or deliberately restore
  keyboard focus after a model update.
- Tab indices are deterministic within each surface.
- At 800×600 and 960×600, and UI scales 0.75, 1.0, 1.5, and 2.0, every required
  control is reachable through layout or scrolling; no essential action is
  clipped without a scroll path.

### Task 4 — Navigation consistency

**Objective.** Make the breadcrumb an authoritative, actionable rendering of
the navigation stack.

**Acceptance.**

- Jupiter Collection displays `Solar System › Jupiter › Moons`.
- Switching from Collection to Info or View displays
  `Solar System › Jupiter`.
- Selecting Io displays `Solar System › Jupiter › Io`.
- Activating an ancestor breadcrumb routes through `SimCommand` and returns
  to the corresponding body/page.
- Search and Browse selection close their transient surface and restore focus
  deterministically.

### Task 5 — Q15 recovery

**Objective.** Provide explicit default restoration plus a non-persisted
startup recovery surface for cue-less saved views.

**Acceptance.**

- `--reset-settings` restores `AppSettings::default()`, persists it, and
  launches normally.
- Settings exposes an accessible `RESTORE DEFAULTS` action routed through
  `SimCommand`.
- A saved state with UI on and Orbits/Labels/Icons off shows exactly one
  cue-recovery notice; activating it restores reviewed presentation defaults.
- The notice does not change persisted settings merely by appearing.
- With UI off, the existing `SHOW UI` affordance remains the only recovery
  control.
- Process-relaunch tests prove exact persistence before reset and reviewed
  defaults after explicit reset.

### Task 6 — High-rate visual integrity

**Objective.** Apply orbit emphasis to a body's complete render aggregate.

**Acceptance.**

- Saturn's sphere, rings, label, and icon share the same emphasis blend.
- At +100 yr/s, Mercury through Saturn leave no residual strobing attached
  geometry while their orbits brighten.
- Reducing the rate restores every attached visual smoothly.
- Picking and f64 propagation remain unchanged.
- The onset toast remains transition-only.

### Task 7 — Render/update efficiency and regression coverage

**Objective.** Remove avoidable whole-tree and per-frame work without changing
the visual/physics contracts.

**Acceptance.**

- Stable settings/layer values do not rebuild an entire surface solely to
  repaint one control; if a rebuild is necessary, scroll/focus are preserved.
- A stable orbit-emphasis blend does not rewrite all materials every frame.
- Paused simulation avoids propagation and secular orbit-path rebuilds when
  time and catalog truth are unchanged.
- Any orbit-geometry cache has a documented render-error bound and a test over
  the supported time/zoom domain; no tolerance is loosened merely for speed.
- Composed interaction tests cover focus, modality, scroll, navigation,
  reset recovery, high-rate Saturn rendering, and LIVE replay.

## Verification gate

Each task is reviewed against this document before implementation and verified
before proceeding to the next task. Final completion requires:

```text
cargo test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p solar-sim --features steam
cargo clippy -p solar-sim --all-targets --features steam -- -D warnings
git diff --check
```

The workspace test count may only increase unless the change log records a
specific justified reduction. No generated catalog, truth fixture,
`ARCHITECTURE.md`, or `AGENTS.md` file is edited.

## Completion record

Implementation followed the task order above, with this document reviewed
before each task:

1. The command boundary now owns View Options, left-panel page/collapse,
   Browse open/expand, committed settings/default restoration, and breadcrumb
   navigation. Replay-v2 records frame wall delta and wall-clock TDB inputs;
   replay-v1 remains accepted.
2. One interaction-context resource gives text editing, Browse, and Settings
   exclusive ownership of keyboard, right-drag, wheel, label activation, and
   viewport picking. Escape has one semantic owner per context.
3. Settings, left-panel content, and all Browse columns use retained,
   clamped scroll positions. Scroll surfaces capture wheel input, modal tab
   navigation is explicit, and keyboard focus is restored to the corresponding
   semantic action after a surface rebuild.
4. The breadcrumb renders the navigation stack as accessible buttons and
   ancestor activation queues `NavigateBreadcrumb`.
5. Settings exposes `RESTORE DEFAULTS`; `--reset-settings` persists defaults
   before normal startup; cue-less visible-UI states expose one transient
   recovery notice without changing persistence by appearance.
6. Under the human Q16 ruling and ARCHITECTURE §10.3, Saturn remains
   text-only: its sphere, rings, text label, and orbit share the emphasis
   transition, with no Saturn icon or reticle. Io supplies the representative
   Icons-layer reticle coverage and uses the same shared emphasis blend.
7. Unchanged clocks no longer trigger body propagation or secular orbit-path
   comparison, and exact orbit-cache keys preserve zero-error geometry reuse.
   Stable emphasis, presentation, settings, Layers, Browse, left-panel,
   search, time-control, and breadcrumb values avoid redundant component,
   material, asset, or UI-tree writes while preserving semantic focus and
   scroll across necessary rebuilds. A composed real-catalog lifecycle covers
   the interaction, recovery, navigation, high-rate aggregate, and LIVE-replay
   boundaries together.

Verification evidence:

```text
cargo test
  331 passed: 53 sim-core + 227 solar-sim + 48 xtask lib
              + 2 xtask smoke + 1 active spot-check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p solar-sim --features steam
  228 passed
cargo clippy -p solar-sim --all-targets --features steam -- -D warnings
git diff --check
```

The final replay portable state hash is `1535747298578131566`. Its intentional
changes during this cycle reflect newly replayed View Options, application
settings, navigation and modal state, followed by the canonical semantic
navigation identity. No tolerance or physics assertion was loosened.

## Post-completion architecture-conformance addendum — 2026-07-17

A subsequent architecture-to-source audit limits the completion record above
to the seven stabilization tasks and their stated acceptance evidence. It does
not waive three implementation conflicts discovered after that closeout:
Layers-panel open/close bypasses `SimCommand`, the application does not expose
the prescribed ARCHITECTURE §8.2 plugin ownership graph, and the top bar places
Menu before Search. WP10's hyperbolic no-period behavior is separately tracked
as justified and requires no immediate source action.

The binding issue descriptions, ordered remediation phases, acceptance
criteria, and automatic post-verification submission policy are recorded in
`docs/ui-gameplay-architecture-conformance-2026-07-17.md`. No corrective source
work is authorized until the human reviews and approves that plan.

The human subsequently approved the corrective plan, and the follow-up is now
complete on `codex/ui-gameplay-remediation`. AC-1 was submitted as `21352b9`,
AC-2 as `da483a1`, and AC-3 as `d35c531`, after the documentation baseline
`cf7aab1`; every commit was pushed in the prescribed order. Layers-panel
visibility now crosses the shared command/replay boundary, application
assembly exposes the ARCHITECTURE §8.2 owner graph, and the top bar presents
and tabs through Search before Menu. The detailed final cross-reference is in
the conformance plan's execution closeout.

The original 331-test/`1535747298578131566` evidence above remains the exact
historical stabilization-cycle result. After the architecture follow-up, the
integrated workspace passes 337 tests (53 `sim-core`, 233 `solar-sim`, 48
`xtask` library, 2 xtask smoke, and 1 active spot-check); Steam-feature mode
passes 234 tests. The current portable replay hash is
`8282160698094571922`, changed only because AC-1 made Layers-panel visibility
canonical and hash-covered. Formatting, both warning-denied Clippy modes, the
16-asset texture metadata audit, and `git diff --check` pass. The justified
hyperbolic-period wording clarification remains deferred, and WP16/WP17 were
not resumed.
