# UI playability optimization design plan (2026-07-23)

**Status:** design only; no source implementation is authorized by this
document.

**Purpose.** This plan translates the eight requested UI changes into
command-safe, testable work while preserving the repository's determinism,
catalog, persistence, accessibility, and licensing rules. It is based on the
current Rev D architecture, the 2026-07-22 UI/performance and decision records,
the current 66-body catalog, and a source audit of the relevant UI systems.
Conflicts and unresolved product choices are called out explicitly rather than
silently resolved in code.

## 1. Executive recommendation

Proceed in seven separately reviewed implementation blocks:

1. define one replayable **Reset Interface** semantic and route every reset
   surface to it;
2. add category-derived orbit widths and a reviewed per-body orbit palette;
3. change new-profile visibility and overview body presentation, then correct
   the body-size scale ordering;
4. replace the Menu's generic shortlist/expand model with the requested fixed
   lists and parent-grouped moon expansions;
5. reproduce and correct the Search integration defect without replacing the
   already-implemented fuzzy search engine;
6. expand all 66 descriptions and add an explicit Wikipedia link field and
   platform action;
7. run integrated responsive, accessibility, replay, persistence, golden, and
   real-GPU acceptance.

Do not begin source work until the blocking decisions in §11 are closed and
the human has made the corresponding edits to `ARCHITECTURE.md`. The
recommended product interpretation is:

- “initial state” means the launch-time gameplay/UI snapshot, not factory
  settings and not a disk write;
- the Sun remains visible as the central reference even though the initial
  category filters show only planets and dwarf planets;
- the second Menu column's **SHOW ALL MOONS** reveals moons of dwarf planets,
  not a duplicate of the first column's planet-moon groups;
- orbit colors are unique base colors; selection/emphasis effects may alter
  brightness or alpha but must not erase the body's base hue;
- overview bodies remain the existing textured 3D spheres, with larger
  category-specific apparent-size floors rather than unrelated circle or
  billboard markers;
- ×1/×10/×50 is applied after the overview visibility floor so every choice
  produces a visible ratio.

## 2. Verified current-state findings

| Requested area | Current implementation | Consequence |
|---|---|---|
| Reset | `SimCommand::ResetView` resets only camera pose, selection, and Sun focus. `RestorePresentationDefaults` separately resets layers/view presentation. | Neither command restores the complete startup UI/session state. A new aggregate semantic is required. |
| Top-left Solar System | The root breadcrumb queues `NavigateBreadcrumb`; it does not reset time, layers, view options, or modal/search state. | Its root action must be rerouted to the same new reset command. Non-root breadcrumb actions remain navigation. |
| Time bar | Play/Pause and Live already share the lower time-bar row. | **RESET INTERFACE** can be inserted between them, subject to narrow-window layout tests. |
| Orbit width | Every retained orbit path uses one `1.5` logical-pixel width. | The requested 3×/2×/1× hierarchy is a small render-style change. |
| Orbit color | The eight planets have individual colors; all dwarfs share one color, all moons another, all asteroids another, and all comets another. | A unique per-body palette changes the Rev D palette contract and needs reviewed data. |
| Initial categories | Factory layer state enables every category. Contextual-moon behavior already hides moons while the Sun is focused. | New-profile defaults can hide Asteroids and Comets while leaving the persisted Moons key/default intact. |
| Body appearance | Bodies are already 3D spheres. The Sun, eight planets, the Moon, the four Galilean moons, and Titan have texture assets. No dwarf planet has a texture asset. | Existing assets satisfy the planet portion only. “Actual” dwarf appearances require new licensed assets or an explicitly representative fallback. |
| Body visibility/scale | Non-Sun bodies have a 3-logical-pixel diameter floor applied after the physical ×1/×10/×50 calculation. | At overview distances, all three choices can collapse to the same 3-pixel result even though the physical-radius math is correct. |
| Menu | Three curated shortlists expand to complete generic category lists through **SHOW ALL**. The first shortlist contains the Sun and selected moons. | The requested fixed lists and grouped moon views replace, rather than merely restyle, the current Menu data model. |
| Search | Case-insensitive exact, prefix, alias, and fuzzy ranking already exist. Results update from editable search state and result activation queues `TravelToBody`. A test proves `jupter` ranks Jupiter first. | The report that Search is not implemented indicates an integration/discoverability defect. Reproduce `jupit` in the real app before changing the algorithm. |
| Descriptions | All 66 bodies have reviewed NASA/JPL-sourced descriptions constrained by a test to 2–4 sentences. `BodyRecord` has description text but no Wikipedia URL. | The 150-word requirement changes the catalog content contract and panel layout load. |

## 3. Reset Interface design

### 3.1 One semantic action

Add `SimCommand::ResetInterface`. Do not assemble the reset by sending a
sequence of UI-local mutations, and do not make the button call
`ResetView` plus `RestorePresentationDefaults`; partial failure or ordering
would make desktop and replay behavior diverge.

At bootstrap, after settings and the startup-rate command have been applied,
capture a deterministic `SessionStartupSnapshot`. `ResetInterface` restores
that snapshot through the normal command reducer. The snapshot contains:

- simulation time, rate, and play/pause state;
- selected body, focused system/body, camera pose/distance, and no active
  travel tween;
- breadcrumb at the root;
- launch-time layer visibility;
- body-size choice, per-system Major/All state, and local-orbit toggles;
- left-panel tab/collapse state;
- Menu closed and its expansions reset;
- Search query/results/dropdown cleared;
- Help, Settings, Layers, recovery notices, and transient toasts closed;
- keyboard/input focus returned to the ordinary viewport;
- UI visibility restored to the launch-time value.

The reset must not:

- overwrite `settings.toml`;
- restore factory defaults when the user launched with persisted settings;
- change the operating-system display mode independently of the launch
  snapshot;
- regenerate the catalog or touch physical/orbital truth.

For a fixed-epoch launch, Reset returns to that epoch. For a Live launch, it
returns to the captured boot instant rather than silently sampling a new wall
clock; the existing Live button remains the action for “now.”

### 3.2 Surfaces

All of these surfaces queue exactly one `ResetInterface`:

- lower time bar: **PAUSE/PLAY · RESET INTERFACE · LIVE**;
- root **Solar System** breadcrumb in the top-left;
- the existing Home-key Reset View intent;
- the Help modal's existing Reset View action, renamed **Reset Interface**.

Non-root breadcrumb segments remain ordinary navigation. The time-bar button
gets accessible label “Reset interface to its launch state.” It is immediate,
with no confirmation dialog, because it does not delete persisted data.

At 800×600 and/or 2.0 UI scale, the visible label may wrap to two lines inside
a fixed minimum hit target, but it may not be shortened for assistive
technology. The required visual order and tab order are Pause/Play, Reset
Interface, Live.

### 3.3 Acceptance

- A table-driven reducer test mutates every captured field, resets, and
  compares the complete state to the startup snapshot.
- The time-bar button, root breadcrumb, Home intent, and Help action each emit
  the same single command.
- Reset while Menu, Help, Settings, Search, UI-off mode, and an in-flight
  camera tween are active lands in the same state.
- Record → serialize → replay produces the same state hash.
- Reset is idempotent.
- Persisted settings bytes are unchanged.

## 4. Orbit hierarchy and color design

### 4.1 Width mapping

Keep the current `1.5` logical pixels as the 1× base:

| Category | Multiplier | Width |
|---|---:|---:|
| Planet | 3× | 4.5 logical px |
| Dwarf planet | 2× | 3.0 logical px |
| Natural satellite | 2× | 3.0 logical px |
| Asteroid | 1× | 1.5 logical px |
| Comet/interstellar comet | 1× | 1.5 logical px |

The Sun has no orbit path. Width is derived from `Category` in one pure
function and does not enter the retained geometry cache key. Distance/angle
fades and high-rate orbit emphasis continue to affect alpha/brightness, not
width.

### 4.2 Palette storage and rules

Add a reviewed `orbit_color_srgb` field to the hand-authored manifest and
emitted `BodyRecord`; do not infer a palette from list position or mutate the
generated catalogs by hand. `color_srgb` remains the body fallback/material
color. This keeps the orbit palette auditable without forcing the body and
orbit to use identical display colors.

Palette rules:

- every one of the 65 orbiting bodies has a unique 24-bit sRGB value;
- hues follow familiar photographic/illustrative cues for the body;
- dark-body colors are luminance-lifted enough to remain visible on the
  space background;
- color is never the only category cue: line width, Menu grouping, labels,
  and accessibility names remain available;
- selected/high-rate states multiply the base color; they do not replace all
  bodies with a common hue;
- an automated duplicate-RGB test is mandatory, plus a generated contrast
  and perceptual-distance review report for human palette sign-off.

### 4.3 Candidate palette for review

These are proposed base orbit colors, not final scientific surface colors.
They are all exact-RGB unique. Icy/gray bodies intentionally stay within their
traditional families; the width and label remain the primary non-color cues.

| Body | Hex | Body | Hex |
|---|---:|---|---:|
| Mercury | `#A8A8A8` | Venus | `#E6C57A` |
| Earth | `#4D8DFF` | Mars | `#D46745` |
| Jupiter | `#D5A66F` | Saturn | `#E8D19A` |
| Uranus | `#72D4DC` | Neptune | `#586FEA` |
| Ceres | `#B9B7B2` | Pluto | `#D5A58A` |
| Eris | `#E8E5DF` | Haumea | `#DDECF0` |
| Makemake | `#B86E52` | Gonggong | `#A64A48` |
| Quaoar | `#A87363` | Orcus | `#8C9199` |
| Sedna | `#8F3E43` | Moon | `#C7C2B8` |
| Phobos | `#8F7A68` | Deimos | `#B6A58A` |
| Io | `#F3D35C` | Europa | `#E8D6A9` |
| Ganymede | `#9C8065` | Callisto | `#6F625A` |
| Amalthea | `#B5534D` | Himalia | `#85817A` |
| Mimas | `#E4E1D9` | Enceladus | `#D7F2FF` |
| Tethys | `#CEDCE6` | Dione | `#C4CED8` |
| Rhea | `#B8B8B2` | Titan | `#D9A43B` |
| Hyperion | `#A88665` | Iapetus | `#8A7864` |
| Phoebe | `#686A70` | Miranda | `#D5D1C9` |
| Ariel | `#C8E1E4` | Umbriel | `#7A7E82` |
| Titania | `#B9C4C6` | Oberon | `#8C8A89` |
| Triton | `#C9B7C4` | Nereid | `#8DA1AF` |
| Proteus | `#696D73` | Charon | `#AAA6A0` |
| Nix | `#D4D0C8` | Hydra | `#C2C8CF` |
| Dysnomia | `#A3ABB5` | Hiʻiaka | `#E4EDF0` |
| Namaka | `#BCCDD2` | 2 Pallas | `#A6AAB0` |
| 3 Juno | `#A77C5A` | 4 Vesta | `#D8C9B0` |
| 10 Hygiea | `#666B70` | 16 Psyche | `#8494A6` |
| 433 Eros | `#B58A61` | 101955 Bennu | `#53575B` |
| 99942 Apophis | `#7D7068` | 1P/Halley | `#78D9F2` |
| 2P/Encke | `#5FC4D6` | 9P/Tempel 1 | `#91D3DD` |
| 67P/Churyumov-Gerasimenko | `#70C9B6` | 103P/Hartley 2 | `#59E0C5` |
| Hale-Bopp | `#8FAEFF` | NEOWISE | `#B5DDF4` |
| 3I/ATLAS | `#74A6C9` |  |  |

### 4.4 Acceptance

- Exact category-to-width unit test covering every catalog category.
- Manifest/catalog validation rejects missing or duplicate orbit colors.
- All 65 paths spawn with their expected reviewed width/color.
- Retained path geometry and f64 propagation are byte-for-byte/state-hash
  unchanged.
- Goldens cover full system, asteroid belt, Jupiter, Saturn, and a comet view
  at normal and high-rate emphasis.
- A color-vision simulation review confirms that width/labels preserve
  comprehension when hues converge.

## 5. Initial screen and body appearance

### 5.1 New-profile defaults

For factory defaults and **RESTORE DEFAULTS**:

| Layer | Initial value |
|---|---|
| User Interface | on |
| Planets | on |
| Dwarf Planets | on |
| Asteroids | off |
| Comets | off |
| Moons | on, but contextual rules render none while focused on the Sun |
| Orbits | on |
| Labels | on |
| Icons | on only where compatible with the visible categories |

The Sun remains visible as the central star/reference. The initial path filter
therefore shows only planet and dwarf-planet orbits. Existing user settings
continue to load exactly; the new default is not silently imposed on returning
profiles.

### 5.2 Replace “small circles” with appearance-bearing bodies

Do not add a second UI circle marker. Continue to render the existing UV
sphere/material aggregate at the body's orbital position:

- planets: reuse the eight existing KTX2 textures;
- Saturn: keep its ring aggregate attached and scaled consistently;
- dwarf planets: add licensed appearance assets where defensible;
- unresolved dwarf planets: use a clearly documented representative albedo
  based on measured color, not a fabricated “actual photograph.”

At overview distances, use category-specific ×1 minimum apparent diameters:

- planets: 12 logical px;
- dwarf planets: 8 logical px;
- other categories: retain 3 logical px when their layers are enabled.

These are starting values for the golden review, not physical radii. The
center remains at the true propagated position; f64 truth, orbital geometry,
and catalog radius remain unchanged. If density causes overlap near the Sun,
the design may reduce only unselected bodies continuously toward 8 px
(planets) / 6 px (dwarfs); it must never move their orbital positions or turn
them into detached icons.

### 5.3 Correct body-size scale semantics

The present formula is effectively:

`max(true_radius × body_size, visibility_floor)`.

This makes ×1, ×10, and ×50 look identical whenever the floor dominates. The
requested visual semantics require:

`max(true_radius, category_visibility_floor) × body_size`.

Apply this to body visuals and Saturn's ring aggregate. Picking remains based
on the existing accessibility/picking radius rather than a ×50 physical hit
sphere, so exaggeration cannot select a body from an unrelated orbit. The
View Options label remains **BODY SIZE** with ×1/×10/×50, and its accessible
description states that this is a visual exaggeration only.

Acceptance captures the same camera/body at all three choices and checks
projected diameter ratios of 1:10:50 within raster tolerance. The test must
include a floor-dominated dwarf, Earth, Saturn and rings, and a close-focused
body where true radius already exceeds the floor.

### 5.4 Asset constraint

The repository currently has no dwarf-planet texture files. Ceres and Pluto
have candidate spacecraft imagery for an asset pipeline, subject to the
normal source/license-sidecar audit, but several distant dwarf planets lack
resolved global surface maps. The product must not describe an artist-created
or color-estimate texture as an “actual appearance.” Q25 records the required
human choice.

## 6. Menu information architecture

Replace generic “shortlist vs all” behavior with a catalog-derived fixed base
view and optional grouped moon sections. Canonical ordering is stored as
reviewed body IDs, while names/counts/parent relationships come from the
catalog.

### 6.1 Column 1 — Planets & Moons

Base list, exactly:

1. Mercury
2. Venus
3. Earth
4. Mars
5. Jupiter
6. Saturn
7. Uranus
8. Neptune

The Sun is absent. Footer: **SHOW ALL MOONS**. When expanded, the footer
becomes **HIDE MOONS** and the following groups appear beneath their parents:

- **Earth** — Moon
- **Mars** — Phobos, Deimos
- **Jupiter** — Io, Europa, Ganymede, Callisto, Amalthea, Himalia
- **Saturn** — Mimas, Enceladus, Tethys, Dione, Rhea, Titan, Hyperion,
  Iapetus, Phoebe
- **Uranus** — Miranda, Ariel, Umbriel, Titania, Oberon
- **Neptune** — Triton, Nereid, Proteus

Mercury and Venus do not get empty headings.

### 6.2 Column 2 — Dwarf Planets & Asteroids

Show all 17 entries immediately, with visible subgroup headings:

- **Dwarf planets (9):** Ceres, Pluto, Eris, Haumea, Makemake, Gonggong,
  Quaoar, Orcus, Sedna.
- **Asteroids (8):** 2 Pallas, 3 Juno, 4 Vesta, 10 Hygiea, 16 Psyche,
  433 Eros, 101955 Bennu, 99942 Apophis.

Recommended footer semantics: **SHOW ALL MOONS** reveals:

- **Pluto** — Charon, Nix, Hydra
- **Eris** — Dysnomia
- **Haumea** — Hiʻiaka, Namaka

The footer then becomes **HIDE MOONS**. Bodies without cataloged moons do not
get empty headings. This is the only interpretation that makes the second
column's action category-local; duplicating all planet moons here would make
the two columns inconsistent.

### 6.3 Column 3 — Comets

Show all eight immediately:

1. 1P/Halley
2. 2P/Encke
3. 9P/Tempel 1
4. 67P/Churyumov-Gerasimenko
5. 103P/Hartley 2
6. Hale-Bopp
7. NEOWISE
8. 3I/ATLAS

The footer retains the same height, separator, background, and spacing as the
other columns, but it is an inert presentational node with no text, button
role, hover state, pointer target, or tab stop. A blank focusable button would
violate the accessibility contract.

### 6.4 Behavior and acceptance

- Selecting any body queues `TravelToBody(id)` and closes the Menu.
- Expansion state remains replayable. The existing
  `SetBrowseColumnExpanded` command may be retained, but its new semantics
  must be documented and the comet column must reject/ignore expansion.
- Parent headings are not body actions unless they are also rendered as the
  normal parent row.
- Keyboard focus order follows visual order, including grouped children.
- Each column scrolls independently; the fixed footer never scrolls away.
- Tests pin exact base order, group order, counts 8/17/8, 26 planet moons,
  6 dwarf moons, absence of Sun, and absence of a comet-footer action.

## 7. Search bodies acceptance and correction

### 7.1 Preserve the existing engine

The current search already provides the intended algorithmic foundation:

- trimmed, case-insensitive input;
- exact name/designation/alias priority;
- prefix matching, so `jupit` should rank Jupiter;
- typo-tolerant fuzzy matching, already tested with `jupter`;
- deterministic ordering and one best hit per body;
- live result state and click-to-`TravelToBody`.

Do not add a second search implementation or network lookup.

### 7.2 Reproduction-first correction

Run a real desktop interaction trace:

1. focus **Search bodies...** with mouse and keyboard;
2. enter `j`, `ju`, `jupit` one character at a time;
3. record query state, editable text, dropdown root, hit list, z-order,
   input-focus owner, and click activation after each frame;
4. click Jupiter and verify the travel command and resulting view;
5. repeat while Menu/Help/Settings have recently opened and closed.

Likely fault classes to test are input focus ownership, dirty-state rebuild,
dropdown z-order/picking, or a stale claim caused by an older build. Change
only the failing integration layer.

### 7.3 Acceptance

- `jupit`, `jupter`, `JUPITER`, `3I/ATLAS`, and at least one catalog alias
  each rank the intended body.
- Results update on the frame after each edit without Enter.
- Empty/whitespace input closes the dropdown.
- At most eight results are shown; keyboard arrows/tab and Enter activate the
  highlighted result; pointer activation remains equivalent.
- Activation queues exactly one `TravelToBody` and closes Search/Menu state.
- Modal/text ownership prevents simulation hotkeys from leaking.
- A real rendered test or controlled UI trace covers the editable-widget
  integration, not only the pure ranking function.

## 8. Description and Wikipedia-link design

### 8.1 Content contract

For all 66 bodies:

- 150–220 words of original, neutral prose;
- at least one paragraph, optionally split into two for readability;
- include classification/location, physical appearance/composition where
  known, orbit/rotation context, discovery or exploration relevance, and one
  distinguishing feature;
- distinguish measured fact, consensus interpretation, and uncertainty;
- no invented appearance details for unresolved bodies;
- one canonical English Wikipedia article URL;
- retain the existing NASA/JPL description source in provenance.

Wikipedia is consulted as the requested secondary reference and direct reader
link. Scientific claims remain cross-checked against the existing NASA/JPL
sources. Representative starting references include
[Jupiter](https://en.wikipedia.org/wiki/Jupiter),
[the natural-satellite list](https://en.wikipedia.org/wiki/List_of_natural_satellites),
and the direct article for each catalog body.

### 8.2 Licensing posture

Wikipedia text is reusable under free/open licenses, but redistribution or
adaptation carries attribution and ShareAlike obligations. Wikimedia's
[Terms summary](https://foundation.wikimedia.org/wiki/Policy%3ATerms_of_Use/Summary/en)
and the
[CC BY-SA 4.0 license](https://creativecommons.org/licenses/by-sa/4.0/)
should be reviewed before content lands.

Recommended approach for this proprietary repository:

- do not copy sentences or closely paraphrase Wikipedia's expression;
- use Wikipedia to identify topics and provide the requested outbound link;
- write original copy from independently verified facts, prioritizing the
  existing NASA/JPL public sources;
- run phrase-similarity and human editorial review;
- if any Wikipedia-derived wording is intentionally reused, isolate it with
  page revision URL, authorship/history attribution, modification notice, and
  license metadata, and obtain a product/legal sign-off before shipping.

This is a risk-control recommendation, not legal advice.

### 8.3 Schema and UI

Add an optional `wikipedia_url` to the hand-authored manifest and emitted
`BodyRecord`, with migration/default behavior for older fixtures. The catalog
validator checks HTTPS, `en.wikipedia.org/wiki/`, non-empty slug, and exactly
one URL per production body. Network availability is not a startup or test
requirement.

The Info panel shows the description in its existing scrollable content and a
focusable **Wikipedia ↗** row below it. Its accessible label is “Open the
Wikipedia article for {body name}.” Activation queues a semantic
`OpenBodyReference(body_id)` command; a platform service resolves only the
validated catalog URL and opens it in the default browser. Raw user-supplied
URLs are never executed.

If browser activation is not authorized, the fallback design is a visible,
selectable URL plus a **COPY LINK** action. Do not implement a fake link that
silently does nothing.

### 8.4 Acceptance

- Exactly 66 descriptions, each with 150–220 whitespace-delimited words and a
  reviewed direct Wikipedia URL.
- The old 2–4-sentence test and comments are deliberately replaced, not
  weakened to accept both contracts.
- Long-copy scroll/wrap tests cover 800×600 and UI scales 0.75, 1.0, 1.5,
  2.0 with the link reachable by pointer, keyboard, and assistive technology.
- URL activation is command-routed, allowlisted, and excluded from headless
  side effects while remaining replay-parseable.
- Catalog generation remains offline and deterministic.
- A source/word-count/link audit artifact is included with the content review.

## 9. Architecture and specification conflicts

| ID | Requested behavior | Current design-of-record | Resolution required |
|---|---|---|---|
| C1 | Every body has a unique orbit color. | Rev D §10.2 specifies per-category defaults, with only planets individually colored. | Human Rev E amendment authorizing catalog-backed per-body orbit colors. |
| C2 | Menu uses fixed full lists and special **SHOW ALL MOONS** behavior. | Rev D §9.1 specifies curated shortlists expandable to complete category lists. | Human Rev E amendment replacing the shortlist contract and defining the second-column moon meaning. |
| C3 | Overview bodies show recognizable appearances at a larger appropriate scale. | Rev D §10.1 fixes a universal 3-logical-pixel non-Sun floor after exaggeration. | Human Rev E amendment for category-specific overview floors and asset truth labels. |
| C4 | ×10/×50 must visibly scale a floor-dominated body. | Rev D §10.1 explicitly applies the minimum apparent size after ×10/×50. | Human Rev E amendment changing formula order to floor first, then exaggeration. |
| C5 | “Actual” dwarf-planet appearances reuse existing assets. | No dwarf-planet texture assets exist, and several bodies have no resolved global surface imagery. | Asset/source decision; requirement cannot be met literally from existing files. |
| C6 | 150+ word descriptions with clickable Wikipedia links. | Catalog/spec comments and tests require 2–4 sentences; schema has no URL. Every user action must be a `SimCommand`; root licensing is proprietary. | Update WP3/WP10 specs/tests, approve URL platform mechanism, and approve the content licensing posture. |
| C7 | Reset from “any screen.” | Modal/input ownership deliberately blocks underlying HUD actions; UI-off mode hides the HUD. | Define whether the Home/Help command path satisfies “any screen” or whether every modal needs a visible reset action. |

The following are compatible without architecture deviation:

- category-derived orbit line widths;
- new-profile Asteroids/Comets defaults off while preserving exact persisted
  settings;
- retaining Moons default ON with contextual rendering at Sun focus;
- routing all reset surfaces through one new `SimCommand`;
- fixing a verified Search integration defect while preserving its current
  semantic travel command.

## 10. Implementation sequence and gates

Each block is a separate TASKS work package/change package. Do not combine
catalog content, renderer styling, and control semantics into one review.

### UIO-0 — Human decisions and Rev E

Close Q22–Q26, edit `ARCHITECTURE.md`, and approve the candidate palette and
asset terminology. No source work before this gate.

### UIO-1 — Reset Interface

Implement the startup snapshot, command serialization/replay, reducer, four
surfaces, accessibility, and state-completeness tests.

### UIO-2 — Orbit hierarchy

Implement category widths, approved manifest orbit colors, catalog validation,
generated-catalog regeneration through `xtask`, and orbit goldens.

### UIO-3 — Initial presentation and body scale

Change only factory layer defaults first. Then implement approved overview
floor/appearance assets and correct scale ordering with projection/ring/picking
tests and reviewed goldens.

### UIO-4 — Menu

Replace the browse model and footer semantics, keeping travel command routing,
independent scrolling, keyboard focus, replayable expansion state, and exact
catalog-derived counts.

### UIO-5 — Search integration

Capture the real failure, patch the smallest failing layer, and add the
rendered/integration regression for `jupit`.

### UIO-6 — Description content and reference links

Land schema/platform mechanics and tests before the 66-body copy pass. Then
add reviewed copy and provenance in batches without changing catalog identity,
ordering, physics, or body count.

### UIO-7 — Integrated playability acceptance

- full workspace and Steam-feature tests;
- fmt and warning-denied clippy;
- catalog dry-run and fixture regeneration as applicable;
- replay/state-hash and settings migration/round-trip gates;
- responsive matrix: 800×600 and 960×600 at 0.75/1.0/1.5/2.0 UI scales;
- keyboard and AccessKit audit;
- canonical Metal goldens plus category-specific views;
- M2 Pro real-app walkthrough: reset from every state, Menu grouping, `jupit`
  travel, all size choices, and outbound/reference fallback behavior;
- record before/after performance numbers because thicker paths and larger
  apparent bodies increase fill/overdraw.

## 11. Open decisions blocking implementation

### Q22 — Reset scope and universal access

Approve the launch-time gameplay/UI snapshot in §3, with no disk write and no
factory reset. Confirm that the global Home/Help command path satisfies “from
any screen”; otherwise every modal needs an additional visible Reset Interface
action.

### Q23 — Menu architecture and second-column moons

Approve replacement of Rev D's curated-shortlist model. Confirm that column
two's **SHOW ALL MOONS** means the six cataloged moons of Pluto, Eris, and
Haumea, rather than duplicating the 26 planet moons from column one.

### Q24 — Unique orbit palette

Approve catalog-backed per-body orbit colors and the candidate table in §4.3,
subject to contrast/perceptual review. This supersedes Rev D's shared
non-planet category colors.

### Q25 — Dwarf appearance and size semantics

Approve the category-specific overview floors and floor-before-exaggeration
formula. Choose one honest dwarf-planet asset policy:

1. public-domain resolved textures where available plus clearly labelled
   representative albedo for unresolved bodies (recommended);
2. representative albedo for all dwarfs for visual consistency;
3. postpone “actual appearance” for dwarfs and ship the planet-only asset
   improvement.

### Q26 — Wikipedia content and external links

Approve original 150–220 word NASA/JPL-cross-checked copy with Wikipedia as a
secondary reference/link, rather than copied/adapted Wikipedia prose. Also
approve either a platform URL-opening implementation (and any dependency or
platform code it requires) or the no-browser fallback of visible URL plus
Copy Link.

## 12. Definition of done

The request is complete only when:

- both reset surfaces and the fallback intent restore one reviewed startup
  state through one command;
- all 65 orbit paths have the correct 4.5/3.0/1.5 logical-pixel width and a
  unique approved base color;
- a new profile initially shows the Sun, planets, dwarf planets, and only
  their orbit paths, with recognizable honest body appearances;
- ×1/×10/×50 is visibly correct at overview and focused distances;
- Menu lists and moon groups match §6 exactly and the blank comet footer has
  no interactive semantics;
- real-time typo-tolerant Search works in the rendered app and travels on
  activation;
- every body has reviewed 150–220 word copy and a working or explicitly
  copyable direct Wikipedia link;
- all architecture amendments, catalog provenance, licensing metadata,
  replay/persistence gates, accessibility checks, goldens, and performance
  evidence are accepted.
