# Game design — insights and recommendations (2026-07-22)

These are product/design observations recorded per request. Nothing here
authorizes source work; items marked *beta* are folded into the UI/performance
plan, items marked *fast-follow* are post-beta candidates, and items marked
*positioning* are store/marketing work with no code dependency.

## 1. Time is the toy — protect and showcase it

The genuinely differentiated verb in this product is not the camera, it is the
±100 yr/s time ladder with honest two-body motion. Everything that makes rate
changes *legible* multiplies the product's value: orbit-emphasis already does
this at high rates, and the R1 startup-rate ruling makes it the first thing a
player experiences. Two consequences worth internalizing. First, any future
feature should be evaluated by whether it degrades time-scrubbing (e.g.,
n-body or ephemeris interpolation would make scrubbing slower and
nondeterministic — reject). Second, "events" are the natural content axis for
this toy: perihelia, oppositions, ring-plane crossings, and close approaches
are all *computable from the existing catalog* and give players destinations
in time, not just space. A "Next events" list that queues `SetTime` commands
is a cheap, high-retention fast-follow that needs no new data source.

## 2. Tours are replays wearing a costume

The demo script (2026 → Sedna → Jupiter → Halley 1986 → +100 yr/s → LIVE)
already exists as a WP17 requirement, and the command/replay machinery makes
any scripted session a first-class artifact. Generalize this into 4–6 curated
**tours** — Grand Tour, Halley's Return, Ring Worlds, The Kuiper Frontier,
3I/ATLAS Flyby — each a recorded command session plus short caption cards
keyed to timeline positions. This is the single highest-leverage content
feature available: it reuses tested machinery, doubles as the WP17 replay
library, produces the Steam trailer footage for free, and solves cold-start
("what do I do?") for the store's most skeptical reviewers. *Fast-follow;
design the caption-card schema before implementing.*

## 3. First-run onboarding should be three sentences, not a wizard

With Escape-Help now shipped, first run needs only a transient three-line
coach mark (drag to orbit · scroll to zoom · the bar at the bottom is time),
dismissed on first interaction and never shown again (settings flag). Anything
heavier fights the product's contemplative identity. *Beta-adjacent; one small
WP, reuse the toast/recovery-notice machinery.*

## 4. Photo mode is nearly free and Steam loves it

UI-off presentation mode already exists. Add a screenshot key that writes a
timestamped PNG (the golden capture path already does window readback) and
you have a "photo mode" bullet point for the store page plus a community
screenshot pipeline. *Fast-follow, small.*

## 5. Sound: ship silent for beta, decide deliberately for 1.0

There is currently no audio content, and an ambient soundtrack drags in
licensing-audit surface (WP17) plus taste risk. Silence is defensible for a
planetarium; a single OFL/CC0 ambient bed with a mute default-off toggle is
the maximum worth considering for 1.0. Do not let audio enter beta scope.

## 6. Positioning and store craft

Position as **"a precision solar-system instrument you can play"** — closer
to a hardware synth than a game. Tags: Space, Simulation, Education, Sandbox,
Relaxing. The six canonical golden views are, deliberately, the screenshot
set. Two claims are credible and provable and should lead the copy: real
orbital mechanics from JPL data (provenance is committed), and the ±100
years/second time ladder. Avoid "NASA" in any branding (licensing audit
already forbids it); "built from JPL Horizons data" is accurate and safe.
Achievements are cheap goodwill later — the `PlatformServices` trait was
built for exactly this; keep them out of beta.

## 7. Scope discipline — the "no" list

Recorded so future enthusiasm has friction: no n-body, no spacecraft/mission
mode, no VR, no multiplayer, no procedural bodies, no constellation *art*
beyond the Q4 line set, no mod support before 1.0. Each violates either the
two-body determinism contract, the 350 MB envelope, or the one-person
maintenance budget. The 66-body catalog is a curated instrument, not a
database — resist "just add one more body" requests except through the
existing manifest + human sign-off route.

## 8. The camera is the character

Eyes' feel comes from its travel tween more than any single visual. Post-beta
polish budget is best spent on: eased dolly with distance-proportional speed
(likely exists), arrival framing that composes parent+moons rather than
centering the target alone, and a subtle idle drift in presentation mode.
These are feel items — schedule them only after reference-hardware perf
evidence exists, since feel work on a 120 Hz M2 Pro can mislead.
