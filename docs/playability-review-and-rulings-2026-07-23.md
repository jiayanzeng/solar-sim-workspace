# Playability review, rulings Q19–Q26, and execution queue (2026-07-23)

**Reviewer:** Claude (technical advisor / quality gate). **Authority:** the
human delegated resolution of open issues to Claude's best judgment on
2026-07-23 ("resolve the issue using the solution you consider best; no need
to ask me"), with hardware explicitly deferred to last. Closures below are to
be transcribed into `TASKS.md` under the normal protocol citing this record.

---

## 1. Verification of the completion report and the July 22 program

Every material claim in Codex's completion report was checked against the
updated source and holds:

- **UIP-2/3/4/5/7 verified in source.** `apparent_size.rs` implements the
  continuous 3-logical-px floor; `settings.rs` carries `startup_rate` with
  normalization and a Settings row; `control.rs` has `RegionPreset` +
  `TravelToRegionPreset` reduced in both desktop and headless paths;
  `orbit_lines.rs` implements the quarter-pixel bounded secular reuse with the
  analytic drift bound. The Rev D paragraphs landed in `ARCHITECTURE.md`
  (startup rate §7-area, contextual Moons §9.3, presets §9, apparent-size
  §10.1, bounded reuse §10.2).
- **Q18 is genuinely closed**, on strong evidence: the UIP-9 Tier-1
  swapchain-acquisition conjunct, 14 consecutive passes across two verified
  login sessions, a deliberately occluded negative control that failed
  loudly without a readback, and explicit human closure.
- **Test baseline 407/408** matches the board; the dashboard is coherent.
- **Ledger transcription gaps confirmed:** Q4 and Q12 still read "open" in
  `TASKS.md` despite the 2026-07-22 delegated closures (D1/D2). §5 below
  authorizes the transcription.
- **Uncommitted state:** the report's note that four Q18-cleanup files are
  modified but uncommitted is accepted as reported; committing them with a
  change-log entry is housekeeping item H1.

**Accountability note.** My D3/UIP-8 ruling was wrong, and the failure mode is
worth recording: the "golden-harness readiness" condition I approved measured
a different capture path (offscreen image, not the window surface) and its 5 s
settle floor made passing and failing runs print identical durations — the
acceptance signal did not measure the binding variable, so five consecutive
passes produced a false green. Codex's D5/UIP-9 correction (an explicit
surface-availability conjunct on the same path, plus a negative control and a
cross-session evidence bar) is the right design and the right standard.
Lesson, binding on future acceptance criteria I author: **evidence must
measure the binding variable on the code path under test, and any repetition
bar must include a negative control.**

**Program status in one line:** the 2026-07-22 UI/performance program is
implemented and integrated except UIP-1/UIP-6's Ultra measurements (Q19) and
the Retina-fullscreen question (Q21); the July 23 playability plan is
design-complete and blocked only on the decisions closed below.

---

## 2. Rulings on the performance stragglers (Q19–Q21)

### Q19 — CLOSED: presets request, the adapter resolves, the UI tells the truth

Ultra keeps `Msaa::Sample8` as its *requested* value, but preset application
now resolves the highest adapter-supported sample count ≤ the request (wgpu's
reported set; `[1, 2, 4]` on the M2 Pro) **before** it reaches the camera, so
the renderer can never enter the unsupported state again. This is an explicit,
displayed clamp, not a silent one: the Settings row shows the effective value
("ULTRA — 8× (4× ON THIS DEVICE)" pattern), and frame-stats output records
both requested and effective counts. The UIP-1 baseline matrix is revised
accordingly: record Ultra-effective; where it equals High's effective value,
one annotated measurement satisfies both rows. Rationale: stable four-preset
UI, zero schema churn, honest per-device behavior, and the `StoppedUnexpected`
failure class is eliminated by construction.

### Q20 — CLOSED: compile the debug-only assertions conditionally

Gate the two `DiagnosticsOverlayState` tests under `cfg(debug_assertions)` so
`cargo clippy --workspace --all-targets --release` compiles. Release all-target
clippy remains an unofficial probe, **not** a CI gate — the two existing
warning-denied debug matrices stay the required standard. One-line-scale
maintenance under UIP-1.

### Q21 — CLOSED: Retina toggle is windowed-scope, by declaration

The observed macOS borderless-fullscreen behavior (winit keeps the swapchain
at native physical resolution regardless of the scale-factor override) is
accepted as a platform reality. The setting is declared **windowed-effective**:
its Settings description gains "takes effect in windowed mode; fullscreen
renders at display resolution," and the limitation is documented. No forced
windowed mode, no internal render-scale chain now. Recorded revisit trigger:
if the (deferred) M1 Air reference gate misses 60 fps and the windowed
Retina-off numbers show resolution is the binding term, the render-scale
chain becomes a measured, justified block. This keeps Q21's remaining work at
one string and one doc note — consistent with hardware-last.

---

## 3. Rulings on the playability decisions (Q22–Q26)

### Q22 — CLOSED: launch-time snapshot approved; "any screen" = one Escape away

The §3 design is approved exactly: one `SimCommand::ResetInterface` restoring
a deterministic `SessionStartupSnapshot` captured after settings and the
startup-rate command apply; no `settings.toml` write, no factory reset; Live
launches reset to the captured boot instant (Live button remains "now"); the
four surfaces (time-bar button between Pause and Live, root breadcrumb, Home
intent, renamed Help action) queue the identical single command; idempotence,
replay-hash, and table-driven completeness tests as specified. On universal
access: the global command path **satisfies** "from any screen." `ResetInterface`
itself closes every modal/search/UI-off state as part of the snapshot restore,
every modal is one Escape from the surfaces, and Home works in UI-off. No
per-modal duplicate buttons — they would churn the modal layout matrix for no
real reachability gain. Document "Reset is reachable within one Escape from
every screen" in Help.

### Q23 — CLOSED: Menu replacement approved; column-2 moons are dwarf moons

The Rev D §9.1 curated-shortlist contract is replaced by the plan's §6 fixed
inventories: column 1 exactly the eight planets (no Sun) with SHOW ALL
MOONS/HIDE MOONS revealing the 26 planet moons grouped under their parents
(no empty headings); column 2 all 17 dwarf planets + asteroids with visible
subgroup headings, its SHOW ALL MOONS revealing exactly the six dwarf-planet
moons (Pluto: Charon/Nix/Hydra; Eris: Dysnomia; Haumea: Hiʻiaka/Namaka) —
Codex's recommendation is confirmed; duplicating planet moons there would
break column locality. Column 3 all eight comets with the inert,
non-focusable styled footer (a blank focusable button would violate the
accessibility contract, as the plan correctly states). Counts 8/17/8 + 26 + 6,
absence-of-Sun, and footer-inertness tests as specified.

### Q24 — CLOSED: per-body orbit palette approved with a perceptual gate and four planet corrections

Catalog-backed `orbit_color_srgb` in the hand-authored manifest is approved,
with `color_srgb` unchanged as body/material fallback. The candidate palette
is approved **subject to a mandatory perceptual gate**, because my audit of the
proposed table found exact-RGB uniqueness (65/65) but visually
indistinguishable pairs: Ceres–Rhea (ΔRGB ≈ 1), Miranda–Nix (≈ 2),
Phoebe–Hygiea (≈ 2), and weak planet-tier separation (Venus–Saturn ≈ 34,
Venus–Jupiter ≈ 37, Earth–Neptune ≈ 38 in RGB distance). Rulings:

1. **Planet-tier corrections (apply to the table):** Venus → `#EDC24F`
   (saturated gold), Jupiter → `#C4854F` (deep orange-tan), Saturn →
   `#F2E3AE` (pale cream), Neptune → `#3C55E0` (indigo-azure). Mercury, Earth,
   Mars, Uranus stand.
2. **Automated gate (test, not review-only):** exact-RGB uniqueness across all
   65; pairwise CIE76 ΔE (Lab) ≥ 25 between any two planets; pairwise
   CIE76 ΔE ≥ 4 between any two bodies. Lab conversion + ΔE76 are ~60 lines of
   pure test math — no dependency. Colors nudged to satisfy the gate must
   stay within their stated traditional family and the three near-identical
   pairs above are mandatory fixes.
3. Width remains the primary hierarchy cue exactly as designed (4.5/3.0/1.5
   logical px from `Category`, outside the geometry cache key); the
   color-vision simulation review stands as final human sign-off input.

### Q25 — CLOSED: category floors, floor-before-exaggeration, asset policy 1

Approved: category-specific ×1 overview floors (planets 12 px, dwarf planets
8 px, others 3 px; continuous, center at true propagated position) and the
corrected formula `max(true_radius, category_floor_equivalent) × body_size`
so ×1/×10/×50 always produces a visible ratio, with the 1:10:50
projected-diameter acceptance including a floor-dominated dwarf, Earth,
Saturn + rings, and a close-focus case; picking stays on the existing
accessibility radius. Dwarf asset policy: **option 1** — public-domain
resolved mission textures where they exist, through the existing
`convert-texture` + provenance-sidecar + metadata-audit pipeline, and a
documented representative-albedo material for unresolved bodies, never
described as an "actual photograph." Beta texture scope: **Ceres and Pluto
(New Horizons / Dawn), plus Charon if the same pass is cheap**; everything
else representative albedo. The density fallback (unselected-body shrink
toward 8/6 px) is approved as designed.

### Q26 — CLOSED: original 150–220-word copy; Wikipedia as link, not source text

Approved exactly as recommended: original neutral prose of 150–220 words per
body, cross-checked against the existing NASA/JPL provenance sources, with
Wikipedia consulted as a secondary reference and provided as the outbound
link; **no copied or closely paraphrased Wikipedia expression** — the CC
BY-SA attribution/ShareAlike obligations must not enter a proprietary corpus,
and linking carries no such obligation. The old 2–4-sentence test is replaced,
not weakened. Schema: optional-in-manifest, exactly-one-per-production-body
`wikipedia_url` with the HTTPS + `en.wikipedia.org/wiki/` + non-empty-slug
validation as specified. **URL opener ruling:** implement the command-routed
`OpenBodyReference(body_id)` with a std-process platform opener (macOS
`open`, Windows `ShellExecute` via `cmd`/`start` with the validated URL only —
no new dependency, catalog-validated URLs only, never user-supplied strings),
routed **through `PlatformServices`** so the WP16 Steam build can later
upgrade to the overlay web page without touching call sites; headless,
golden, and replay contexts get the no-op service. Spawn failure surfaces the
visible-URL + COPY LINK fallback via the existing toast machinery — never a
dead link.

### Two cross-interaction rulings the plan missed

These close real playability holes created by the new Asteroids/Comets-off
defaults and are added to the relevant acceptance lists:

- **R-NAV (navigation reveals hidden categories).** Any `TravelToBody` —
  from Search, Menu, breadcrumb, or collection rows — targeting a body whose
  category layer is hidden queues one explicit
  `SetLayerVisibility(category, true)` before the travel command. Otherwise a
  new profile that searches "Halley" arrives at empty space, and the WP17
  demo script breaks on its own defaults. Command-routed and recorded, so it
  is not a silent override; the Layers panel reflects the change.
- **R-PRESET (Belt preset reveals the belt).** `TravelToRegionPreset(Belt)`
  queues `SetLayerVisibility(Asteroids, true)` when hidden, same mechanism.
  The other three presets stay pure framing (their regions are populated by
  default-visible categories).

---

## 4. Rev E — exact ARCHITECTURE edits (human pastes, one commit)

Apply before UIO-2/3b/4/6 start. Suggested commit: "ARCHITECTURE Rev E:
delegated playability rulings Q22–Q26 (2026-07-23)".

1. **§9.1 (top bar / Menu), replace the browse-page sentence with:** "Menu →
   full-screen browse page with three fixed catalog-derived columns: the
   eight planets (SHOW ALL MOONS expands the 26 planet moons grouped under
   their parents), all dwarf planets and asteroids under visible subgroup
   headings (SHOW ALL MOONS expands the six dwarf-planet moons), and all
   comets (inert styled footer). Live counts derive from the catalog; the
   Sun does not appear in the lists."
2. **§9 (interface), append:** "One `ResetInterface` command restores the
   launch-time session snapshot (time, rate, play state, camera, selection,
   breadcrumb, layers, view options, panel/modal/search state, focus, UI
   visibility) without writing settings; it is surfaced between Pause and
   Live, on the root breadcrumb, on the Home key, and in Help."
3. **§9.3 (Layers), append:** "Factory defaults start Asteroids and Comets
   off. Navigation to a body in a hidden category, and the Belt region
   preset, enable the relevant layer through an explicitly queued
   `SetLayerVisibility` command."
4. **§10.1 (bodies), replace the minimum-apparent-size sentence with:**
   "Render-only category minimum apparent diameters at ×1 — planets 12,
   dwarf planets 8, all other non-Sun bodies 3 logical px — are applied
   before the optional ×10/×50 exaggeration
   (`max(true_radius, floor) × scale`), so exaggeration is always visible;
   physical truth, picking, and orbits are unaffected. Dwarf-planet surfaces
   use public-domain resolved textures where available and documented
   representative albedo otherwise."
5. **§10.2 (orbit paths), replace the color sentence with:** "Per-body
   reviewed orbit colors from the catalog's `orbit_color_srgb` field (unique
   across all 65 orbiting bodies, perceptual-distance-gated), with
   category-derived line widths of 3×/2×/1× a 1.5-logical-px base for
   planets / dwarf planets and moons / asteroids and comets;
   distance/angle alpha fades and high-rate emphasis modulate brightness and
   alpha, never the base hue."
6. **§4.1 / §5 note (schema), append:** "Schema additions: reviewed
   `orbit_color_srgb` and validated `wikipedia_url` per production body;
   descriptions are 150–220 words of original provenance-checked prose with
   a `Wikipedia` reference action routed through `PlatformServices`."

---

## 5. Housekeeping (H-items, one small change package)

- **H1** — commit the four modified Q18-cleanup files with a change-log entry.
- **H2** — transcribe closures into the ledger: Q4 (D1, 2026-07-22), Q12 (D2,
  2026-07-22), Q19/Q20/Q21/Q22/Q23/Q24/Q25/Q26 (this record, 2026-07-23).
- **H3** — append D7 (Q19–Q21 rulings) and D8 (Q22–Q26 rulings, R-NAV,
  R-PRESET, palette corrections) to `docs/decision-record-2026-07-22.md` or
  reference this file from it, so the decision trail stays in one place.

---

## 6. Execution queue (hardware last, playability first)

Codex's UIO block structure is adopted with a resequencing: Search is the
worst user-facing defect (reported as "not implemented") and needs no
architecture edit, so it goes first, and everything that can run before Rev E
does. One active package at a time, normal submission standard throughout.

**Wave 0 — no Rev E required, start immediately:**
1. **H1+H2+H3** housekeeping (minutes, unblocks a clean ledger).
2. **UIO-5 — Search reproduction and fix.** Reproduction-first exactly per
   plan §7; patch the smallest failing integration layer; rendered/trace
   regression for `jupit`. The engine is not to be rewritten.
3. **UIO-1 — Reset Interface** per Q22.
4. **UIO-3a — factory defaults + R-NAV + R-PRESET.** Asteroids/Comets off
   for new profiles and RESTORE DEFAULTS; hidden-category navigation reveal;
   Belt-preset reveal; persisted profiles untouched.
5. **UIP-6/1 maintenance mini-block** — Q19 effective-MSAA resolution +
   Settings/frame-stats surfacing, Q20 cfg fix, Q21 description string;
   then complete the UIP-1 baseline matrix with Ultra-effective rows.

**Rev E gate:** human applies §4 above (mechanical paste, one commit).

**Wave 1 — after Rev E:**
6. **UIO-2 — orbit hierarchy** (widths + corrected palette + ΔE test gate +
   catalog regeneration through xtask + goldens).
7. **UIO-3b — overview appearance and scale** (category floors,
   floor-before-exaggeration, Ceres/Pluto/Charon texture pass under the
   existing provenance pipeline, representative-albedo materials, density
   fallback, 1:10:50 acceptance).
8. **UIO-4 — Menu replacement** per Q23.
9. **UIO-6 — descriptions + Wikipedia links** per Q26 (schema/platform
   mechanics first, then the 66-body copy pass in batches).
10. **UIO-7 — integrated playability acceptance** per plan §10, including
    the before/after perf capture (thicker lines + larger floors increase
    fill; the frame-stats baseline from Wave 0 is the comparator).

**Parked (hardware-last, unchanged):** D4 purchases, WP16 resume, WP17
gates, the on-site Windows day, and the Q21 render-scale revisit trigger.
Nothing in Waves 0–1 depends on them.
