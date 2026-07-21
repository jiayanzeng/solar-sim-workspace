# UI/gameplay request — architecture review and correction outline

**Audit date:** 2026-07-18

**Status:** Documentation-only review; no source change is authorized or made by
this report.

**Design of record:** `ARCHITECTURE.md` Rev C.

**Request reviewed:** The twelve UI/gameplay observations submitted after the
WP5 → WP8 → WP14 corrective cycle.

## 1. Purpose and decision rule

This report determines whether each observation is:

1. an implementation defect against Rev C;
2. a compatible enhancement that can be designed inside Rev C;
3. content or visual-polish debt; or
4. a requested product change that conflicts with, or is not sufficiently
   defined by, Rev C and therefore needs a human architecture ruling before
   source work.

The review applies these binding boundaries:

- every action remains `input/UI → SimCommand → canonical reducer`
  (`ARCHITECTURE.md` lines 74–77 and 327–331);
- simulation truth remains f64 heliocentric kilometres, with one
  floating-origin f32 render conversion and parent-relative orbit vertices
  (lines 78–85 and 333–338);
- the time ladder, LIVE predicate, fixed epoch, and snap-to-LIVE behavior remain
  the frozen `sim-core` contract (lines 139–168);
- UI stays first-party Bevy UI, AccessKit-labelled, and compatible with the
  retained `ui_kit` façade (lines 340–350);
- Layers grouping, body-size options, body rendering, labels/reticles, picking,
  and selection behavior remain governed by lines 360–415;
- behavior changes require tests and a green complete workspace gate (lines
  436–456).

## 2. Executive disposition

| # | Requested outcome | Disposition | Can implementation begin without a ruling? |
|---|---|---|---|
| 1 | Movable view, pointer camera control, reset view | Implementation gap / compatible enhancement | Yes, after a WP5 plan |
| 2 | Clearer initial bodies, classic silhouettes, non-Sun scaling | Partial conflict with true-radius/exaggeration rules; comet tails are unspecified | No for new scaling/tail rules |
| 3 | Highlight selected body and its satellite paths | Compatible render-only enhancement | Yes |
| 4 | Stop apparent Neptune-moon drift at zoom | Probable label/reticle projection defect, not orbital mechanics | Yes |
| 5 | Selectable and highlighted orbit paths | Compatible enhancement; picking algorithm is unspecified | Yes, after algorithm is recorded |
| 6 | Additional mouse/keyboard aliases and region presets | Aliases compatible; preset semantics underspecified | Yes for aliases; no for presets until specified |
| 7 | Escape overview/help guide | Compatible UI enhancement | Yes |
| 8 | Default launch rate of 1 day/s | Conflicts with the frozen +REAL startup behavior/current LIVE contract | No |
| 9 | Remove Moons from Layers and auto-show selected-system moons | Direct conflict with the exact §9.3 Layers model and persistence/replay schema | No |
| 10 | Centered, larger bottom time controls | Compatible layout enhancement | Yes |
| 11 | Replace letter controls with drawn icons | Compatible visual/accessibility enhancement | Yes |
| 12 | Longer body descriptions | Existing content debt against the 2–4 sentence schema intent | Yes, through the curated manifest |

The three requests that need an explicit architecture/product decision before
implementation are #2, #8, and #9. The region-preset portion of #6 needs a
smaller behavior specification before it is deterministic enough to implement.

## 3. Finding-by-finding review

### 3.1 View movement and reset

**Requested behavior.** Dragging the background should move the view in every
direction, mouse input should change the camera view, and a reset control should
restore the initial view.

**Architecture result.** Rev C defines an orbit camera around a moving focus,
not translation of the rendered background. Adding another pointer gesture and
an explicit reset command is compatible. A free background/world translation
that bypasses the camera controller would violate the f64/floating-origin and
one-command-path boundaries.

**Current source evidence.**

- `crates/solar-sim/src/input_intent.rs:520` emits orbit input only while the
  right mouse button is held; wheel dolly is handled in the same intent layer.
- `crates/solar-sim/src/control.rs:29` owns the semantic `SimCommand` variants,
  and the camera reducer owns orbit/dolly state.
- `crates/solar-sim/src/lib.rs:900–929` constructs the initial Sun-focused,
  full-system camera framing.
- There is no explicit `ResetView` command or retained initial-pose reset path.

**Correction outline.**

1. Add left-drag as an alias for orbit, with a movement threshold so a normal
   primary click remains body/orbit selection.
2. Retain right-drag as an existing compatible input.
3. Add an explicit `SimCommand::ResetView` that restores the reviewed startup
   focus, yaw, pitch, and full-system distance through the shared reducer.
4. Expose the same command from a labelled UI button and a documented key.
5. Preserve HUD/modal/text-edit pointer ownership and replay serialization.

**Acceptance criteria.**

- Left- and right-drag over the viewport emit the same orbit command shape;
  dragging over HUD, a modal, or editable text emits no gameplay command.
- A primary click below the drag threshold still selects exactly one target.
- Reset restores the canonical initial pose through one command and produces
  the same desktop/headless replay state hash;
- focus, travel, dolly clamps, floating-origin precision, and existing camera
  tests remain green.

### 3.2 Initial body visibility and classic visual characteristics

**Requested behavior.** Bodies other than the Sun should be clearly visible at
startup, retain features such as rings and comet tails, and avoid both uniform
tiny dots and arbitrary disproportionate sizes.

**Architecture result.** This is only partly compatible. Rev C requires UV
spheres at true radius multiplied by one of the optional visual exaggerations
and explicitly supports the full-system vastness shot (`ARCHITECTURE.md`
lines 333–338 and 393–398). The current ×1/×10/×50 options apply to all body
spheres; Rev C does not grant a special “exclude the Sun” scale rule. Making
every physical sphere clearly resolved in one full-system frame cannot be
achieved with true relative radii alone. Saturn rings are required; comet tails
are not specified in Rev C.

**Current source evidence.**

- `crates/solar-sim/src/lib.rs:1245–1287` spawns every sphere at true radius
  and attaches Saturn's ring aggregate.
- `crates/solar-sim/src/left_panel.rs:344–370` defines ×1, ×10, and ×50, with
  ×1 as the default at lines 387–395; render scale is computed at lines
  496–498.
- `crates/solar-sim/src/surface_textures.rs:74–139` implements Saturn's
  textured annulus.
- No comet-tail render system or tail asset exists in current source.

**Required ruling.** Define whether the product wants an adaptive visual proxy,
an approved non-Sun-only scale, a different default exaggeration, or a camera
composition change. Separately define comet-tail geometry, orientation,
length/brightness inputs, layer ownership, and high-rate behavior. None should
be invented during implementation.

**Acceptance criteria after a ruling.**

- f64 positions, physical radii, picking truth, and orbit geometry are never
  modified by visual scaling;
- the Sun remains exactly as directed by the ruling and Saturn retains its
  architecture-preserving sphere/rings/text/orbit aggregate with no planet
  reticle;
- representative planet, dwarf, asteroid, moon, and comet visuals remain
  identifiable at the approved canonical views without overlapping UI;
- visual proxy/tail behavior has deterministic golden coverage at all required
  viewport/UI scales and at normal/high rates;
- no new asset or dependency enters without its normal provenance/sign-off.

### 3.3 Selected-system highlight

**Requested behavior.** Selecting a body with satellites should highlight the
body and the body/satellite orbit paths with one clean visual treatment.

**Architecture result.** Compatible when selection remains canonical camera
state and the accent is render-only. It must compose with orbit alpha fades,
Layers, Major/All moon filtering, local orbit visibility, and high-rate
emphasis; it must not rebuild stable orbit geometry merely to recolor it.

**Current source evidence.** `crates/solar-sim/src/orbit_lines.rs:261–297`
stores palette and displayed color state; lines 412–506 update retained orbit
assets only when geometry or color inputs change. There is no selected-system
accent input. Selection is already available from `CameraController` and label
priority uses it in `crates/solar-sim/src/labels.rs:506–578`.

**Correction outline.** Add a selection-accent render input derived from the
selected body and catalog parent/child topology, then blend sphere/ring and
eligible orbit colors without changing f64 state or the path cache.

**Acceptance criteria.**

- selecting a parent accents the parent, its orbit, and the visible orbits of
  its catalog children; selecting another target restores exact base colors;
- hidden layers/moons stay hidden and high-rate alpha remains authoritative;
- stable frames perform no component/asset writes and reuse orbit handles and
  vertex buffers;
- replay/state hashes are unchanged because the effect is render-only.

### 3.4 Apparent Neptune satellite drift

**Requested behavior.** Neptune's satellites, and all other satellites, must
remain correctly positioned on their orbits at every zoom level.

**Architecture result.** This is most likely a presentation defect, not an
orbital or floating-origin defect. The body and orbit paths are independently
derived from the same f64 orbital elements and parent frame. The visible
“moon” at distant zoom is usually its circular UI reticle, and current
decluttering moves the entire label root—including that reticle—away from the
projected body.

**Current source evidence.**

- `crates/solar-sim/src/lib.rs:377–443` composes each moon's parent-relative
  `state_at` result onto its parent's heliocentric f64 state.
- `crates/solar-sim/src/orbit_lines.rs:123–172` samples the same current
  elements/mean-motion path, and lines 412–516 re-anchor parent-relative
  vertices without adding the parent twice.
- `crates/solar-sim/src/labels.rs:134–179` may move focused-system moon labels
  through six rings of alternative rectangles.
- The reticle is a child of the moved root (`labels.rs:270–315`), and the root
  is placed at the chosen declutter rectangle (`labels.rs:475–604`). At zoom
  levels where the true sphere is subpixel, the displaced reticle can be
  mistaken for the moon itself.

**Correction outline.** Keep each reticle centered on the exact projected body
position; move only text during declutter. If displaced text needs a
relationship cue, use a deterministic leader line. Run the check across every
modeled moon system, not only Neptune.

**Acceptance criteria.**

- the reticle center differs from `world_to_viewport(body)` by no more than the
  reviewed subpixel tolerance across a zoom/view matrix;
- declutter can move text without moving the reticle or pick target;
- every moon's propagated point lies on the corresponding retained orbit to the
  independent math tolerance at multiple epochs;
- Neptune/Triton/Proteus/Nereid plus retrograde/high-eccentricity fixtures show
  no apparent detachment through the supported zoom range;
- the fix preserves Rev C's Q16 ruling: planets remain text-only, while Io can
  continue to prove Icons-layer reticle blending.

### 3.5 Selectable orbit paths

**Requested behavior.** Visible orbit paths should be clickable and highlight
when selected.

**Architecture result.** Compatible. Rev C only specifies label and sphere
picking, so orbit picking needs a recorded deterministic screen-space rule.
The cleanest current model is for a path click to select/travel to its owning
body, avoiding a second simulation selection state.

**Current source evidence.** `crates/solar-sim/src/labels.rs:792–850` chooses a
deterministic nearest inflated sphere and queues the existing travel command.
`crates/solar-sim/src/orbit_lines.rs` has retained paths and visibility/color
state but no pointer-picking surface or hit-test.

**Correction outline.** Project visible retained path segments into the
viewport, compute the nearest point-to-segment distance within a reviewed
logical-pixel threshold, then choose the closest candidate with catalog index
as the final tie-breaker. Queue the owning body's existing travel command and
reuse the selected-system accent.

**Acceptance criteria.**

- only visible, nonzero-alpha paths are candidates;
- hit-testing uses the retained displayed path and remains deterministic under
  zoom, floating-origin changes, ties, and overlapping paths;
- HUD/modal/text-edit ownership blocks the viewport hit-test;
- a hit queues exactly one existing body-travel command; a miss queues none;
- tests cover ellipse seam, hyperbolic endpoints, overlapping paths, hidden
  layers, and exact tie-breaking without changing orbit geometry.

### 3.6 Additional controls and region presets

**Requested behavior.** Add left-drag orbiting; arrow-left deceleration/rewind,
arrow-right acceleration, arrow-down reset to 1 day/s, Space pause/resume;
retain all existing controls; add single-click Inner/Outer/Asteroids/Kuiper
region teleportation.

**Architecture result.** Additional input aliases are compatible because they
can map to existing semantic commands. Space already toggles pause/resume.
Current rate stepping is `[`/`]`. Region presets are compatible in principle
but not sufficiently specified: a preset needs a stable destination or
framing rule, yaw/pitch, selection/focus effect, navigation/breadcrumb effect,
and replay representation.

**Current source evidence.** `crates/solar-sim/src/input_intent.rs:55–101`
contains the table-driven keyboard map; `[`/`]` and Space appear at lines
73–93. No ArrowLeft/ArrowRight/ArrowDown entries exist. Raw orbiting is
right-button-only at line 520.

**Correction outline.** Add aliases in the same input table: left/right arrows
map to the existing signed ladder step commands, down maps to the approved
1-day/s `SetRate`, and left drag follows §3.1. Define presets in documentation
before source changes, then route them through a stable semantic command rather
than directly moving the camera.

**Acceptance criteria.**

- every old and new key produces exactly one command in ordinary viewport
  context and none while a modal/text edit owns input;
- rate stepping skips zero, saturates at ±100 years/s, and rewind remains
  signed; ArrowDown selects exactly +1 day/s without altering play state unless
  the approved spec says otherwise;
- preset commands serialize/replay portably, land on the exact documented
  focus/framing, and keep breadcrumb/selection state coherent;
- the help surface and README/source-of-truth key table agree.

### 3.7 Escape overview and operation guide

**Requested behavior.** Escape on the main screen should show a concise game
overview and controls guide.

**Architecture result.** Compatible as a retained Help modal under
`HudPlugin`, provided it follows the existing single-owner Escape priority and
command path. It must not open while Escape is reverting text or closing a
higher-priority Browse/Settings surface.

**Current source evidence.** `crates/solar-sim/src/input_intent.rs:493–505`
gives Escape to current interaction context and deliberately excludes Escape
from the general key table; tests pin Browse-before-Settings and text-edit
ownership. There is no Help modal.

**Correction outline.** Add `OpenHelp`/`CloseHelp` commands and a scrollable,
focus-trapped, AccessKit-labelled modal. The main-screen Escape fallback opens
Help only when no edit/modal/UI-recovery state already owns Escape.

**Acceptance criteria.**

- one Escape action has exactly one owner in every context;
- Help content lists the actual current controls/features and is fully keyboard
  reachable at every required viewport/UI scale;
- opening, closing, focus restoration, and scroll state are deterministic and
  command-routed;
- gameplay input is suppressed while Help is open.

### 3.8 Default speed of 1 day/s

**Requested behavior.** New/default launches should immediately run at
1 day/s.

**Architecture result.** This conflicts with the frozen as-built clock
behavior. `SimClock::new` starts playing at `RateIndex::REAL`, and LIVE is
defined specifically as playing +REAL near wall time (`ARCHITECTURE.md`
lines 162–168). A fixed-epoch default at +1 day/s may be a reasonable product
change, but it cannot be silently introduced under the current contract.

**Current source evidence.** `crates/sim-core/src/time.rs:424–437` starts both
fixed and live clocks at `RateIndex::REAL`; tests pin that result. Settings
constructs the clock through the same path at
`crates/solar-sim/src/settings.rs:404`.

**Required ruling.** The least disruptive specification would be: default
fixed-epoch startup uses +1 day/s; `StartMode::Live`, LIVE snap landing, and an
explicit Live action remain +REAL; reset/default migration semantics are
defined. Rev C and its frozen test contract must be human-updated first if this
is approved.

**Acceptance criteria after a ruling.**

- brand-new fixed-epoch settings start playing at exactly +1 day/s;
- Start Live and completed LIVE snap remain exactly +REAL and satisfy the
  existing LIVE predicate;
- persisted user rate/start-mode behavior and reset/migration behavior match
  the ruling without a hidden clock override;
- calendar, replay, range, pause, high-rate emphasis, and transition-only toast
  tests remain green.

### 3.9 Remove the Moons layer and show moons contextually

**Requested behavior.** Remove Moons from Layers; hide satellites in the
initial view and reveal them automatically for the selected parent.

**Architecture result.** Direct conflict. Rev C §9.3 explicitly requires a
separate Moons group and persisted layer state (`ARCHITECTURE.md` lines
380–384). The current enum, settings snapshot, replay slug, grouping, and tests
encode that exact contract. Contextual label visibility already exists, but
body/orbit visibility is also controlled by the global Moons layer and the
Major/All per-system option.

**Current source evidence.** `crates/solar-sim/src/layers.rs:50–105` defines
`LayerId::Moons`; lines 815–890 render the exact four groups. The state also
controls moon-category visibility. `crates/solar-sim/src/left_panel.rs:372–493`
provides the separate per-system Major/All model.

**Required ruling.** A human architecture revision must define initial moon
sphere/orbit/label visibility; selected parent vs selected moon behavior;
Major/All interaction; old settings and replay migration; browse/search
travel; and UI-off/Icons/Labels behavior. Removing only the row would leave
hidden persisted and replay state with ambiguous effect.

**Acceptance criteria after a ruling.**

- no orphaned `moons` persisted key, replay command, enum variant, tab order,
  or stale accessibility label remains;
- a documented migration safely maps existing settings/replays or explicitly
  rejects an unsupported schema version;
- initial, selected-parent, selected-moon, search-travel, Major, and All states
  have exact body/orbit/label visibility tests;
- the 66-body catalog remains unchanged and all actions stay command-routed.

### 3.10 Centered bottom controls and larger rate slider

**Requested behavior.** Center the time controls in the middle third of the
bottom edge, leave the side thirds visually open to the scene, and enlarge the
slider.

**Architecture result.** Compatible. Rev C requires the time bar behavior but
does not require a full-width opaque surface. The existing bottom-left toast
and bottom-right Layers placement still need reserved collision space.

**Current source evidence.** `crates/solar-sim/src/time_bar.rs:216–303` creates
a full-width bottom bar. The slider is full bar width with a 4 px track and
14 px thumb at lines 388–445.

**Correction outline.** Use a full-width pass-through layout host with one
centered interactive dock; only the dock should own pointer input. Enlarge the
track/thumb/hit target using theme tokens and responsive maximum/minimum widths.

**Acceptance criteria.**

- the dock is centered and bounded at every required 800/960×600 ×
  {0.75, 1.0, 1.5, 2.0} case, with side regions continuing to route viewport
  input;
- date, time, play/pause, Live, rate label, all 24 detents, and keyboard focus
  remain reachable and ordered;
- active toasts and an open Layers panel do not overlap the dock;
- slider mapping remains bit-identical and its visible/hit dimensions meet the
  approved minimum.

### 3.11 Right-rail icons

**Requested behavior.** Use a gear for Settings, macOS-style fullscreen
corners, and three stacked squares for Layers; remove unattractive initial
letters.

**Architecture result.** Compatible visual polish. Code-defined vector/line
geometry avoids font-dependent Unicode, a new icon dependency, and platform
glyph differences. Accessibility must continue to come from semantic labels,
not icon recognition.

**Current source evidence.** `crates/solar-sim/src/layers.rs:659–725` uses
`L`, `FS`/`EX`, and `S` text glyphs even though accessible labels and actions
are already correct.

**Correction outline.** Extend the stable rail-button façade to accept a
code-defined icon scene, implement stacked squares, enter/exit fullscreen
corners, and a gear, and retain the existing action/tab/accessibility model.

**Acceptance criteria.**

- no letter placeholder or Unicode-only glyph remains for the three actions;
- enter/exit fullscreen have visually distinct but semantically stable icons;
- icons remain legible in default/hover/focus/active/disabled states at every
  supported UI scale;
- action routing, focus restoration, AccessKit labels, and tab order remain
  unchanged; no new dependency is added.

### 3.12 More detailed body descriptions

**Requested behavior.** Provide materially richer descriptions for every body.

**Architecture result.** This is known content debt. The schema documents a
curated 2–4 sentence Info description, but empty values are currently only
lint warnings. The committed catalog has 66 description fields: 45 are empty
and the remaining 21 are generally one sentence.

**Current source evidence.** `crates/sim-core/src/catalog.rs:182–185` states
the 2–4 sentence intent and the lint appears at lines 649–652. The authoring
source is `xtask/src/manifest.rs:106–123`; `assets/catalog.ron` is generated
and must never be edited by hand.

**Correction outline.** Research and author all blurbs in the curated
manifest with per-body provenance, review them for neutral factual wording,
then regenerate the catalogs only through `xtask gen-catalog`. Treat layout
and accessibility as part of the content pass.

**Acceptance criteria.**

- all 66 bodies have a reviewed 2–4 sentence description and the empty-
  description lint count is zero;
- every factual claim has an auditable source note and no prohibited branding
  or unsupported superlative is introduced;
- generated catalogs exactly reflect the manifest and pass loader/validation,
  composition, ordering, and spot-check tests;
- long descriptions wrap and scroll without clipping at every required
  viewport/UI scale and remain exposed to accessibility.

## 4. Recommended execution order

The work should not be solved as twelve isolated patches. Subject to the
required rulings, use this integrated order and keep exactly one `TASKS.md`
work package active at a time:

1. **Projection integrity:** fix reticle/text separation and run an all-system
   body/orbit/reticle alignment sweep (#4).
2. **Camera and discoverability:** left-drag alias, deterministic reset,
   arrow aliases, and Escape Help under WP5/HUD ownership (#1, alias portion
   of #6, #7).
3. **Selection interaction:** selected-system accent and orbit hit-testing,
   sharing one selection/render model (#3, #5).
4. **HUD polish:** centered responsive time dock, larger slider, and semantic
   code-defined right-rail icons (#10, #11).
5. **Content:** 66-body description/provenance pass with regenerated catalogs
   (#12).
6. **Human-ruling queue:** initial visibility/visual proxies and comet tails
   (#2), region-preset semantics (#6), +1 day/s startup (#8), and replacement
   of the Moons layer contract (#9).

Each phase must begin by rereading `ARCHITECTURE.md`, `TASKS.md`, this complete
report, and the relevant WP brief. Its source and tests land together; the full
default and applicable feature gates must pass before any phase is submitted.

## 5. Submission standard for any later implementation

For every authorized phase:

1. record the phase start, reviewed constraints, exact scope, and any new Open
   question in `TASKS.md` before editing source;
2. do not edit `ARCHITECTURE.md`, any `AGENTS.md`, generated catalogs, or
   spot-check truth data;
3. preserve the command queue, replay schema/version rules, f64 truth,
   floating-origin conversion, stable retained render assets, and accessibility
   contracts;
4. add behavior/regression tests in the same change and never weaken existing
   assertions to obtain a pass;
5. pass `cargo test`, format check, warning-denied clippy, applicable feature
   tests/clippy, metadata/data gates, and `git diff --check`;
6. update the phase record with acceptance evidence and confirm no unrelated WP
   or held Steam work was touched;
7. submit only after every phase-specific criterion is green. Any architecture
   conflict is returned as an Open question instead of being improvised.

## 6. Scope statement

This report records analysis and a correction outline only. It does not reopen
a work package, approve any architecture revision, change a product default,
or authorize implementation. The separately held WP16 Steam stash is outside
scope and must remain isolated.
