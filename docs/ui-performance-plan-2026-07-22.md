# UI design & performance optimization plan (2026-07-22) — TOP PRIORITY

**Authority.** The human delegated closure of the pending design rulings and
selection of the optimization approach on 2026-07-22 ("proceed according to
your recommended approach without needing to consult me"). Rulings R1–R4 below
close the four items the 2026-07-18 review held for human decision (#2, #6
presets, #8, #9). ARCHITECTURE remains human-edited: §7 of this document lists
the exact Rev D text for the human to paste and commit before agents start
UIP-2…UIP-5. Everything else requires no further human input.

**Ground rules carried forward.** All invariants of the 2026-07-18 review §5
apply to every block: command-queue-only mutation, f64 truth, one floating-
origin conversion, retained render assets, tests land with source, one active
WP at a time, TASKS.md protocol, no edits to protected files.

---

## Part A — Design rulings (what "UI meets expectations" now means)

### R1 — Startup rate: +1 day/s (closes item #8)

**Ruling.** New/default launches play at **+1 day/s**. Mechanism: a new
settings field `startup_rate` (ladder value; factory default +1 day/s) that the
app applies **through one recorded `SimCommand::SetRate`** immediately after
clock construction when the session begins. `sim-core` stays frozen:
`SimClock::new` still constructs at +REAL and its pinned tests do not change.
`StartMode::Live` seeding, the LIVE predicate, LIVE-snap landing, and the
explicit Live action remain exactly +REAL; a default boot therefore starts one
action away from LIVE with the LIVE chip un-lit, which is the honest state.
`RESTORE DEFAULTS` and `--reset-settings` restore `startup_rate` to +1 day/s.
Settings schema migration: missing field deserializes to the default.

**Why.** A +REAL boot renders a visually static sky; every new player's first
impression is "nothing works." At +1 day/s the Moon visibly moves within
seconds and Jupiter's moons within a minute — the cheapest possible "the
simulation is alive" signal, with zero contract breakage because the change is
a recorded startup command, not a hidden clock override.

### R2 — Moons become contextual; the layer schema is untouched (closes #9)

**Ruling.** `LayerId::Moons`, its persisted key, its replay slug, and the
Layers-panel row are **retained unchanged** — no schema migration this close
to release. What changes is the rendered semantic of the Moons layer when ON
(default ON): moon **spheres and orbits** render only for the **focused
system** (same focus-system rule the labels already use), still subject to the
per-system Major/All option and local-orbit toggles. Moons layer OFF hides all
moons everywhere, as today. Search/browse travel to a moon focuses its system
and therefore reveals it before arrival.

**Why.** This delivers the requested experience — a clean initial full-system
view, moons appearing when you visit their parent — while touching only
render-side visibility rules plus tests. The review's migration hazards
(orphaned persisted keys, replay schema version, enum removal) are avoided
entirely because nothing is removed.

### R3 — Initial visibility, silhouettes, and tails (closes #2)

**R3a — Minimum apparent size clamp (beta).** Every non-Sun body's rendered
sphere is scaled up, render-only, so its projected diameter never falls below
**3.0 logical px** (computed from true radius × current exaggeration ×
camera distance; clamp applies after ×1/×10/×50). f64 positions, physical
radii, picking truth (already inflated-sphere based), orbit geometry, and the
Sun are untouched. This is the standard planetarium answer to "everything is
an invisible dot" and removes the motivation for a special non-Sun scale rule,
which is therefore **not** adopted. Golden coverage at the canonical views is
required.

**R3b — Silhouettes.** Saturn's ring aggregate is sufficient for beta. No new
silhouette assets.

**R3c — Comet tails: deferred to post-beta fast-follow.** Recorded spec sketch
so no one invents it later: two billboard lobes per comet (dust: broad,
curved-optional-straight for v1, warm white; ion: narrow, anti-sunward, blue),
orientation anti-sunward in the parent frame, length and opacity a power law
of heliocentric distance with onset ≤ 4 AU, suppressed in orbit-emphasis
mode, no new dependencies, own WP with golden coverage. Not beta-blocking;
beta ships without tails.

### R4 — Region presets (closes the preset half of #6)

**Ruling.** Add `SimCommand::TravelToRegionPreset(RegionPreset)` with
`RegionPreset ∈ {Inner, Belt, Outer, Kuiper}` — serialized and replayed like
every other command. Semantics: focus = Sun, selection unchanged, breadcrumb
resets to the root, yaw/pitch = the canonical startup pose, camera distance
fixed per preset to frame heliocentric radii of **1.8 AU (Inner), 3.6 AU
(Belt), 35 AU (Outer), 55 AU (Kuiper)** (exact km constants in the block),
travel via the existing eased tween. Surfaces for beta: number keys **1–4**
in the ordinary-viewport key table, four entries in the Help modal, and four
rows at the foot of the Menu browse page. No new floating HUD cluster — that
keeps the layout matrix stable.

---

## Part B — Performance program

### What we actually know (and don't)

Recorded facts: 92.2 fps at 5120×2880 (MSAA4 High default + bloom + HDR,
below the 120 cap → GPU-bound at 5K on M2 Pro); 120.0 fps at 3456×2168 (cap-
limited, ceiling unknown); 202.7 fps in an earlier lighter-scene windowed run.
Scene cost structure from source review: 66 spheres + rings + one 5,000-star
retained quad mesh + ≤66 retained gizmo orbit paths (256–768 vertices each) +
~66 UI label nodes with a cheap greedy declutter — draw-call and CPU-sim cost
are trivial. The dominant cost is almost certainly **fill: native-retina
resolution × MSAA4 × HDR × bloom chain**. One verified CPU inefficiency:
the orbit-path cache key embeds `elements_at(t)`, so the **8 secular-rate
planets resample their full path (~256–320 Kepler solves each) and rewrite
their GizmoAsset every frame while displayed time advances** — by design
("no time buckets"), but it is measurable, scales with the weakest CPU, and
is avoidable with a bounded-error reuse rule.

No frame-time profile exists on any machine. Therefore the program is
measurement-first: UIP-1 lands before any lever, and every later block must
attach before/after numbers from it.

### Decision rules (recorded now so nobody improvises later)

1. If the M1 Air (once acquired) misses 60 fps all-layers at High, the shipped
   default preset becomes **Medium**; High/Ultra remain user-selectable. No
   architecture change needed — defaults are settings.
2. The GTX 1650 gate runs at 1920×1080; the M1 Air gate runs at native
   2560×1600 **and** the numbers at "Retina off" are recorded alongside.
3. No optimization may change replay state hashes, golden baselines beyond
   its own approved re-baseline, or any f64 truth path.

---

## Part C — Codex prompt blocks (execute in order, one at a time)

Each block is self-contained. Standard preamble for every block: *Read the
root `AGENTS.md`, the complete `ARCHITECTURE.md` (Rev D once committed),
`TASKS.md`, `docs/ui-gameplay-request-architecture-review-2026-07-18.md`, and
this plan before source work. Record the phase start in TASKS.md; land tests
with source; full default + steam-feature gates green before submission;
anything ambiguous becomes an Open question, not an improvisation.*

### UIP-1 — Frame-time instrumentation (no ruling required; start immediately)

**Goal.** Make performance measurable and regression-visible without shipping
overhead.

**Build.** (1) An opt-in `--frame-stats <seconds>` CLI mode that runs the
ordinary app, samples per-frame CPU time via Bevy's frame-time diagnostics,
and on exit prints and writes (path via `--frame-stats-out`) min/mean/p95/p99
frame time, fps, frame count, resolution, MSAA, quality preset, vsync,
frame-cap, and adapter info as one machine-readable line + a small CSV of the
raw series. (2) A hidden runtime overlay (default off, toggled by a documented
debug key, excluded from goldens) showing instantaneous/mean frame time.
(3) An `xtask perf-report` subcommand that formats one or more stats files
into the WP17 evidence table.

**Out of scope.** GPU timestamp queries; Tracy/tracing features; any change to
rendering, defaults, or shipped behavior when the flag is absent.

**Acceptance.** Flag absent → byte-identical behavior (test: no resource
inserted); flag present → deterministic output schema (golden-tested format);
overlay never appears in golden captures; baseline table recorded in TASKS.md
for the M2 Pro at the six canonical views × {Low, Medium, High, Ultra}.

### UIP-2 — Minimum apparent size clamp (R3a; needs Rev D §10 edit)

**Build.** In the render-side body-visual system, compute each non-Sun body's
projected diameter from true radius × exaggeration × camera distance ×
projection, and scale the sphere's render transform so the projected diameter
is ≥ 3.0 logical px; smooth (no per-frame popping — clamp is a continuous
`max`). Picking, f64 state, orbit geometry, Saturn ring aggregate parenting,
and the Sun are untouched.

**Acceptance.** Unit tests for the clamp math at boundary distances; a
regression proving picking targets and replay hashes are unchanged; golden
re-baseline of affected canonical views reviewed against the perceptual gate;
every catalog body's projected size ≥ 3 px in the full-system view test
camera; ×10/×50 still multiply beneath the clamp.

### UIP-3 — Contextual Moons semantics (R2; needs Rev D §9.3/§10 edit)

**Build.** Change moon sphere/orbit visibility to: `Moons layer ON` ∧ (body's
system == focused system) ∧ Major/All ∧ local-orbit rules. Labels keep their
existing contextual rules. Layers panel row, persisted key, replay slug, enum:
unchanged.

**Acceptance.** Exact visibility tests for: initial full-system view (no moon
spheres/orbits), focus Jupiter (its moons appear per Major/All), focus a moon
(system stays revealed), Moons OFF (nothing anywhere), search-travel to
Triton (Neptune system revealed on arrival), replay hash equality for a
recorded pre-change session that never toggles Moons is **not** required —
render truth changed, command truth didn't; state-hash tests must still pass
because visibility is render-side. Settings round-trip unchanged.

### UIP-4 — Startup rate +1 day/s (R1; needs Rev D §7 edit)

**Build.** Add `startup_rate` to `AppSettings` (ladder-valued, default
+1 day/s, normalized into the valid ladder, missing-field-defaults migration);
on session start, enqueue one `SimCommand::SetRate(startup_rate)` through the
ordinary queue after clock construction; expose the field in Settings with
RESTORE DEFAULTS coverage; `--reset-settings` path included.

**Out of scope.** Any change to `sim-core::time`, the LIVE predicate, snap
behavior, or `StartMode` variants.

**Acceptance.** New-profile boot plays at exactly +1 day/s and the LIVE chip
is un-lit; Start-Live + snap remain +REAL and satisfy the LIVE predicate;
the startup SetRate appears in recordings and replays deterministically;
settings schema round-trip + migration tests; all existing time tests green
unmodified.

### UIP-5 — Region presets (R4; needs Rev D §9 edit)

**Build.** `RegionPreset` enum + `SimCommand::TravelToRegionPreset` with the
R4 semantics and the four framing distances as named constants (km, derived
from 1.8/3.6/35/55 AU); keys 1–4 in the ordinary-viewport table; Help modal
entries; four Menu-browse footer rows routed through the same command.

**Acceptance.** Each preset lands on the exact documented focus/pose/distance
(reducer test), serializes/replays portably, clears breadcrumb to root,
leaves selection unchanged; keys emit nothing while a modal/text edit owns
input; Help/README key tables agree; desktop/headless hash parity.

### UIP-6 — GPU levers: quality preset composition (measurement-gated)

**Build.** (1) Extend `QualityPreset` mapping: Low = MSAA off + bloom off +
scale-factor override 1.0 ("Retina off"); Medium = MSAA2 + bloom on; High =
MSAA4 (unchanged); Ultra = MSAA8. (2) A separate explicit "Retina rendering"
toggle (macOS-meaningful; on Windows it is a no-op at scale 1.0) so High can
be combined with non-retina on weak Macs. (3) All changes flow through the
existing ApplySettings path.

**Out of scope.** An internal 3D render-scale target chain (revisit only if
UIP-1 numbers on reference hardware show these levers insufficient).

**Acceptance.** Before/after UIP-1 numbers for each preset on the M2 Pro
recorded in TASKS.md; settings round-trip/migration tests; bloom-off Low
passes the sun-bloom golden's replacement or is excluded by an approved
golden-matrix note; no default changes in this block (decision rule 1 governs
defaults later, on reference-hardware evidence).

**Measurement-gated execution notes (2026-07-22).** Two measurement-gated
issues remain human decisions and are tracked as `TASKS.md` Q19/Q21. First,
the M2 Pro supports only 1x/2x/4x sampling for the HDR color and depth
formats, so the specified 8x Ultra mapping fails renderer validation and
produces no valid frame-stats
report; agents must not silently clamp it. Second, Bevy accepts Low's 1.0
scale-factor override, but macOS borderless fullscreen retains the native 5K
swapchain; agents must not force windowed mode or introduce the out-of-scope
internal render-scale chain without a human ruling.

### UIP-7 — Bounded-error temporal reuse for secular orbit paths

**Build.** Replace exact-equality reuse for **secular** orbits only: retain
the sampled path while a conservative analytic bound on vertex displacement
implied by the element drift since the cached key — evaluated in km via
(|Δa|·(1+e) + a·(|Δe|·(1+e)) + a(1+e)·(|Δi|+|ΔΩ|+|Δω|) in radians) — stays
under **0.25 logical px** at the current camera scale; resample when
exceeded. Non-secular orbits keep exact-equality (they never drift). Document
in the module header that this supersedes the "no screen-space approximations"
note by ruling.

**Acceptance.** Property test: rendered vertices never deviate from a fresh
resample by more than the bound; per-frame retained-asset write count at
+1 yr/s drops from 8 to 0 in stable-camera frames until the bound trips
(counted with the existing `count_retained_writes` pattern); goldens
unchanged (fixed views resample deterministically); UIP-1 before/after CPU
frame-time delta recorded.

### UIP-8 — Smoke readback readiness (implements the Q18 ruling)

**Build.** Rework `--assert-nonblack`: after the frame-count measurement
window completes, the readback is requested only once the **same readiness
condition the golden harness uses** (all referenced textures ready) reports
true, with a hard 10 s readiness timeout that **fails loudly** (distinct exit
message) rather than retrying; the nonblack assertion itself is unchanged and
still one-shot. Print the readiness wait duration so slow-readiness is a
detectable signal, per the black-frame-retry principle.

**Acceptance.** The exact WP17 command `--smoke 60 --expect-backend metal
--reject-software-adapter --assert-nonblack` passes repeatedly (≥5 consecutive
local runs) on the M2 Pro with a nonblack readback; a forced-timeout test
proves the loud-failure path; no retry loop exists; Q18 is then closed in
TASKS.md citing this evidence.

**Execution note (2026-07-22).** The ruled gate, forced-timeout path, and
one-shot behavior are implemented. Although the exact command produced an
initial five-pass M2 Pro/Metal streak, two later runs of the same fresh release
binary returned black after the shared readiness condition reported true at
5.000 and 5.001 seconds. Q18 therefore remains open: Bevy 0.19 exposes an
occluded-surface path that can leave the capture buffer unwritten, but D3 did
not authorize adding surface availability to the readiness rule or imposing a
foreground-window procedure. No retry, added delay, or assertion weakening is
permitted without a human ruling.

---

## Part D — Required ARCHITECTURE Rev D edits (human pastes, commits, done)

Apply before UIP-2…UIP-5. Suggested single commit: "ARCHITECTURE Rev D:
delegated rulings R1–R4 (2026-07-22)".

1. **§7 (time semantics), append:** "A settings-owned startup rate (factory
   default +1 day/s) is applied at session start through one recorded
   `SetRate` command. `StartMode::Live` seeding, the LIVE predicate, and LIVE
   snap remain exactly +REAL; a default boot therefore starts near, but not
   at, LIVE."
2. **§9.3 (Layers), amend the Moons sentence to:** "Moons (default on) gates
   contextual moon presentation: moon spheres and orbits render only for the
   focused system, subject to the per-system Major/All option; off hides all
   moons. The persisted key, replay slug, and panel row are unchanged from
   Rev C."
3. **§9 (interface), append:** "Four region presets — Inner, Belt, Outer,
   Kuiper — are semantic travel commands (focus Sun, canonical pose, fixed
   framing distances of 1.8/3.6/35/55 AU), surfaced as keys 1–4, Help entries,
   and Menu rows."
4. **§10.1 (bodies), append:** "Render-only minimum apparent size: every
   non-Sun sphere is scaled so its projected diameter is at least 3 logical
   px, applied after the optional ×10/×50 exaggeration; physical truth,
   picking, and orbits are unaffected. Comet tails are a specified post-beta
   fast-follow (see the 2026-07-22 plan, R3c)."
5. **§10.2 (orbit paths), append:** "Secular paths may reuse retained
   geometry under a conservative sub-quarter-pixel screen-space drift bound;
   non-secular paths reuse exactly."
