# UI/gameplay architecture-conformance plan — 2026-07-17

## Purpose and authority

This document records the post-stabilization conformance audit requested by
the human maintainer. `ARCHITECTURE.md` remains the design of record and is
read-only. This plan does not reinterpret or revise it. Where implementation
and architecture differ without an explicit human ruling, the implementation
must be corrected.

No source work begins under this plan until the human reviews and authorizes
it. Execution then proceeds in the order below, with exactly one `TASKS.md`
work package in progress at a time. Before each phase, the implementer must
re-read the cited architecture clauses, `TASKS.md`, and this complete plan.

## Audit disposition

### Tracked follow-up — no immediate action

1. **Hyperbolic orbital-period omission — justified.** WP10 explicitly derives
   orbital period from `Orbit::period_s` and specifies that hyperbolic bodies
   show no period. Its acceptance criterion requires a nonempty period only for
   elliptic bodies. Because an unbound hyperbolic trajectory has no orbital
   period, the current 3I/ATLAS behavior is intentional and needs no source
   change in this cycle. Retain this item in the to-do list as a deferred
   documentation clarification: only a human may clarify ARCHITECTURE §9.2 to
   say "orbital period where defined."

### Confirmed non-compliance

1. **AC-1 — Layers-panel visibility bypasses `SimCommand`.**
   `RailAction::ToggleLayersPanel` directly mutates
   `RailUiState.layers_panel_open`. This conflicts with ARCHITECTURE §3.4 and
   the stabilization command-boundary rule that every application-visible
   widget transition crosses `SimCommandQueue`. No approved transient-state
   exception or human ruling exists.
2. **AC-2 — The §8.2 plugin ownership graph is not represented by the
   application.** The runtime preserves the principal system-set order, but it
   does not expose the prescribed `TimePlugin`, `CameraPlugin`, `ScenePlugin`,
   `HudPlugin`, `SearchMenuPlugin`, `SettingsUiPlugin`, and `PlatformPlugin`
   ownership boundaries. Work-package staging explains the implementation's
   current split but does not authorize a different architecture.
3. **AC-3 — Search and Menu are reversed in the top bar.** ARCHITECTURE §9.1
   specifies logo/name, breadcrumb, Search, then Menu. The BSN scene currently
   inserts Menu before Search, and no responsive-layout ruling authorizes that
   reversal.

The accepted Q16 ruling remains binding throughout this work: Saturn stays
strictly text-only, with no icon or reticle, and Io retains representative
Icons-layer coverage. Deferred WP16 Steam/overlay work remains out of scope.

## Ordered corrective phases

### Phase 0 — Authorize and submit the documentation baseline

**Coordinating status.** Documentation only; no work package is reopened.

**Issue addressed.** The stabilization completion record and dashboard do not
currently disclose AC-1 through AC-3 or the deferred hyperbolic clarification.

**Implementation steps.**

1. Review this plan against ARCHITECTURE §§3.4, 8.2, and 9.1–9.4.
2. Confirm that the classifications, phase order, acceptance criteria, and
   automatic submission policy below are approved.
3. Keep all source files unchanged.
4. On human authorization, commit and push the documentation baseline before
   beginning Phase 1.

**Acceptance criteria.**

- `TASKS.md` contains the four-item conformance queue in this order: deferred
  hyperbolic clarification, AC-1, AC-2, AC-3.
- The prior stabilization document links to this plan and limits its completion
  claim to the work it actually verified.
- No work-package status, acceptance checkbox, source file, generated asset,
  truth fixture, `ARCHITECTURE.md`, or `AGENTS.md` file changes.
- `git diff --check` passes.

**Submission.** This phase is the sole review hold. After the human authorizes
the plan, stage only the relevant documentation, commit it with a documentation
message, push the current `codex/` branch to GitHub, and then begin Phase 1.

### Phase 1 — Route Layers-panel visibility through the command boundary

**Coordinating work package.** WP11 only.

**Architecture.** §3.4 (one command path), §3.7 (determinism), §8.2 (input/UI
to command queue), §9.3–§9.4 (Layers panel and right rail), §12 (replay tests).

**Issue addressed.** The panel-open widget directly mutates desktop-only UI
state, so the action is absent from replay and headless state convergence.

**Implementation steps.**

1. Reopen WP11 as `in-progress` and record the pre-code architecture review in
   the `TASKS.md` change log.
2. Add an explicit desired-state `SimCommand` for Layers-panel visibility;
   avoid a context-dependent toggle command so duplicate delivery is
   idempotent and replay order is unambiguous.
3. Put the canonical open/closed value in a presentation state consumed by
   both desktop and headless reducers. The widget observer may enqueue only;
   it must not mutate the canonical state.
4. Extend replay serialization, parsing, rejection coverage, combined state
   hashing, and desktop/headless convergence for the new command.
5. Render the panel from canonical state while preserving retained entity
   identity, scroll position, semantic focus restoration, UI-off recovery, and
   modal precedence.
6. Remove or replace tests that assert direct mutation. Do not weaken any
   existing focus, reachability, persistence, or command-order assertion.

**Acceptance criteria.**

- Activating the Layers rail control queues exactly one explicit-state command
  and performs no direct canonical-state mutation.
- Duplicate open or close commands are idempotent; ordered open/close sequences
  converge identically in desktop and headless execution.
- Replay v2 round-trips the command, rejects corrupt rows, and reproduces the
  same combined state hash. Replay v1 remains parseable.
- Panel focus and scroll survive necessary rebuilds; UI-off still exposes only
  `SHOW UI`; Browse and Settings remain modal owners.
- A grep/static regression prevents reintroducing direct production mutation
  from the rail observer.
- All phase and repository submission gates pass.

**Automatic submission.** Once the acceptance evidence is green, update WP11
and the change log, commit the Phase 1 code/tests/docs, and push the current
branch to GitHub without requesting a separate submission approval. If any
criterion fails or an architectural ambiguity appears, do not commit or push;
record an Open question and report the stop.

### Phase 2 — Restore the prescribed plugin graph and ownership

**Coordinating work package.** WP4 only, as owner of application assembly and
frame flow. Existing feature implementations remain in their original modules.

**Architecture.** §8.2 in full, plus §§3.4, 3.6–3.8, 8.3–8.5, and §12.

**Issue addressed.** Correct behavior is spread across internal plugins and
direct `build_app` registrations without the architecture-named ownership
boundaries.

**Implementation steps.**

1. Reopen WP4 only after Phase 1 is submitted, and record a complete mapping
   from every §8.2 plugin responsibility to its current system/resource owner.
2. Introduce the prescribed application-facing plugin boundaries:
   `TimePlugin`, `PropagationPlugin`, `OriginPlugin`, `CameraPlugin`,
   `LabelsPlugin`, `ScenePlugin`, `OrbitLinesPlugin`, `SelectionPlugin`,
   `UiKit`, `HudPlugin`, `SearchMenuPlugin`, `SettingsUiPlugin`,
   `PlatformPlugin`, and feature-gated `SteamPlugin`.
3. Preserve focused internal plugins as private implementation details where
   useful, but make each architecture-facing plugin the sole composition owner
   for its documented responsibilities. Do not register a system twice.
4. Preserve the chained frame flow exactly: input/UI → commands → clock →
   propagation → origin → camera → labels/UI.
5. Preserve Steam overlay initialization before Bevy graphics-device creation,
   default-build Steam isolation, settings bootstrap ordering, golden capture,
   and render recovery. If the prescribed ownership cannot coexist with one of
   these binding constraints, stop and add an Open question instead of
   inventing a new graph.
6. Add plugin-assembly tests that prove single installation, required resource
   availability, system-set order, and unchanged 66-body/scene/UI startup.

**Acceptance criteria.**

- The application-facing plugin graph matches the names, responsibilities, and
  ordering in ARCHITECTURE §8.2.
- Each runtime system is installed exactly once; no duplicate body, camera,
  label, orbit, HUD, settings, starfield, or renderer-recovery surface appears.
- The existing frame-order test remains exact, propagation remains f64,
  rebasing remains the sole f64→f32 boundary, and the portable replay result is
  unchanged by this structural phase.
- Normal and `steam` feature configurations compile and test; the default
  dependency tree still excludes Steamworks.
- Existing golden-view definitions, Q15 recovery, Q16 Saturn/Io behavior, and
  responsive UI tests remain unchanged and green.
- All repository submission gates pass.

**Automatic submission.** Once the acceptance evidence is green, update WP4
and the change log, commit the Phase 2 code/tests/docs, and push automatically.
Failure or ambiguity stops the phase before submission and becomes an Open
question.

### Phase 3 — Restore Search-before-Menu interface order

**Coordinating work package.** WP7 only.

**Architecture.** §§8.4 and 9.1, plus §3.4 for both controls' actions.

**Issue addressed.** Visual and keyboard order currently place Menu before
Search.

**Implementation steps.**

1. Reopen WP7 only after Phase 2 is submitted and re-review the top-bar BSN,
   focus order, responsive sizing, search/dropdown ownership, and Browse focus
   return.
2. Place Search before Menu in the BSN child order and align semantic tab order
   with the visible order.
3. Preserve the flex-integrated scrollable breadcrumb and the existing
   800×600/960×600 reachability behavior across UI scales 0.75–2.0.
4. Add a structural layout regression asserting logo/name → breadcrumb → Search
   → Menu, plus keyboard activation/focus-return coverage.

**Acceptance criteria.**

- Visual child order and keyboard tab order both match ARCHITECTURE §9.1.
- Search remains fuzzy, case-insensitive, alias-aware, and Enter travels to the
  top hit through `SimCommand`.
- Menu still opens the full-screen Browse surface and Browse close/selection
  restores focus deterministically.
- No required control clips without a scroll path at any supported viewport or
  UI scale.
- Accessibility labels remain present and unique for Search and Menu.
- All repository submission gates pass.

**Automatic submission.** Once the acceptance evidence is green, update WP7
and the change log, commit the Phase 3 code/tests/docs, and push automatically.
Failure or ambiguity stops before submission and becomes an Open question.

### Phase 4 — Integrated conformance closeout

**Coordinating status.** Documentation and verification only; all reopened work
packages must already be closed or handed back before this phase.

**Implementation steps.**

1. Re-read ARCHITECTURE §§3, 7–10, and 12 and repeat the architecture-to-source
   cross-reference for AC-1 through AC-3.
2. Run the complete normal and Steam-compatible verification matrix below.
3. Update `TASKS.md`, this plan, and the stabilization addendum with exact test
   totals, replay-hash disposition, commit identifiers, and pushed-branch
   evidence.
4. Leave the justified hyperbolic clarification in the deferred to-do list and
   leave WP16/WP17 statuses unchanged.

**Acceptance criteria.**

- AC-1 through AC-3 have source and regression-test evidence satisfying the
  authoritative clauses; no undocumented exception remains.
- No test count decreases without a written numerical/behavioral justification.
- No generated catalog, spot-check truth fixture, dependency, catalog
  composition, `ARCHITECTURE.md`, or `AGENTS.md` change occurs.
- The working tree contains no unintended files or unrelated user changes.

**Automatic submission.** After the full closeout gate passes, commit and push
the final documentation/evidence update automatically, then notify the human
with the branch, commits, exact verification totals, and any still-deferred
items.

## Repository submission gate

Every source phase and the final closeout must pass, in this order:

```text
cargo test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p solar-sim --features steam
cargo clippy -p solar-sim --all-targets --features steam -- -D warnings
scripts/check-texture-metadata.sh
git diff --check
```

Targeted tests for the active phase run before the full matrix. No test may be
deleted, skipped, or weakened to make a phase pass. A phase is confirmed
complete only when every acceptance criterion and command is green and its
`TASKS.md` evidence is written. After the human authorizes this plan, that
confirmation is also authorization to stage only the phase's files, create one
focused commit, and push the current branch to GitHub automatically. Creating a
pull request, merging, publishing a release, or resuming deferred WP16 remains
outside this authorization.

## Execution closeout — 2026-07-17

**Status: complete.** The human authorized this plan, and all corrective phases
were executed, accepted, committed, and pushed in order on
`codex/ui-gameplay-remediation`:

1. Documentation baseline — `cf7aab1` (`docs: plan architecture conformance
   remediation`).
2. AC-1 / WP11 — `21352b9` (`fix: route layers panel through commands`).
3. AC-2 / WP4 — `da483a1` (`refactor: restore architecture plugin graph`).
4. AC-3 / WP7 — `d35c531` (`fix: restore search before menu order`).

The final architecture-to-source cross-reference found no remaining exception:

- **AC-1 / ARCHITECTURE §3.4.** `SimCommand::SetLayersPanelOpen(bool)` is the
  explicit desired-state command. `activate_rail_action` only enqueues it;
  `consume_presentation_command` is the shared desktop/headless reducer.
  Replay-v2 serialization, parsing, corrupt-row rejection, combined hashing,
  idempotence, a static observer boundary test, and ordered desktop/headless
  convergence all cover the transition. Replay v1 remains accepted.
- **AC-2 / ARCHITECTURE §8.2.** Application assembly now exposes
  `TimePlugin → PropagationPlugin → OriginPlugin → CameraPlugin →
  LabelsPlugin`, followed by `ScenePlugin`, `OrbitLinesPlugin`,
  `SelectionPlugin`, `UiKit`, `HudPlugin`, `SearchMenuPlugin`,
  `SettingsUiPlugin`, and `PlatformPlugin`; feature-gated `SteamPlugin` still
  initializes before `DefaultPlugins`. The assembly regression pins that
  order, uniqueness, one owner for every focused helper, and required
  resources. Existing scene tests independently pin one 66-body scene, one
  camera, 66 labels/57 reticles, 65 orbits, and the exact frame-set chain.
- **AC-3 / ARCHITECTURE §9.1.** The top-bar BSN child order is logo/product
  name, breadcrumb, Search, then Menu. Search uses `TabIndex(100)` and Menu
  `TabIndex(101)`. A resolved-layout regression pins direct-child order and
  geometry, semantic tab order, and distinct accessibility labels; Search,
  Browse focus return, and supported viewport/UI-scale suites remain green.

Final verification from pushed source commit `d35c531`:

```text
cargo test
  337 passed: 53 sim-core + 233 solar-sim + 48 xtask lib
              + 2 xtask smoke + 1 active spot-check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p solar-sim --features steam
  234 passed
cargo clippy -p solar-sim --all-targets --features steam -- -D warnings
scripts/check-texture-metadata.sh
  passed: 16 KTX2 assets
git diff --check
```

The final portable replay state hash is `8282160698094571922`. Its only change
during this conformance cycle occurred in AC-1 because Layers-panel visibility
became canonical presentation state covered by the combined hash; AC-2 and
AC-3 were behavior-preserving for replay. No test was removed or weakened, and
no dependency, generated catalog, truth fixture, catalog composition,
numerical tolerance, `ARCHITECTURE.md`, or `AGENTS.md` file changed. Q15
recovery and Q16's text-only Saturn/Io-reticle ruling remain intact.

The hyperbolic orbital-period wording clarification remains a justified,
deferred documentation item with no source action. WP16 and WP17 remain under
their existing deferred human/hardware gates; this cycle did not resume Steam
overlay or packaging implementation.
