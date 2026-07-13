# Open questions — research brief & recommendations (2026-07-12)

Decision briefs for `TASKS.md → Open questions`. Each section ends with a
recommendation; the decision and any resulting ARCHITECTURE edits are
yours (agents only prepare diffs). Intended location:
`docs/open-questions-brief-2026-07-12.md`.

---

## Q1 — Bevy 0.19.x minimum Rust toolchain

**Status: answered; close on landing the pin.**

Queried the crates.io API on 2026-07-12:

| version | declared `rust_version` | published |
|---|---|---|
| bevy 0.19.0 | **1.95.0** | 2026-06-19 |
| bevy 0.19.0-rc.3 | 1.95.0 | 2026-06-10 |
| bevy 0.19.0-rc.2 | 1.95.0 | 2026-05-22 |
| bevy 0.19.0-rc.1 | 1.95.0 | 2026-05-13 |
| (bevy 0.18.x) | 1.89.0 | — |

**Action:** `rust-toolchain.toml` with `channel = "1.95.0"` (per
ARCHITECTURE §8.1: pin the minimum); keep `sim-core`'s
`rust-version = "1.75"` claim in its own Cargo.toml. Patch releases of
Bevy 0.19 may raise `rust_version` — the invariants CI will surface that
as a build failure at `cargo update` time, which is the right moment to
revisit the pin.

---

## Q2 — TNO GM values (curated review)

The three placeholder values are in fact the standard literature values,
so this is mostly a citation-and-sign-off exercise — with one genuine
decision hiding inside (Pluto).

| Body | Manifest value | Literature | Source |
|---|---|---|---|
| Eris | 1108 km³/s² | System mass (1.66 ± 0.02)×10²² kg → GM_sys ≈ 1108 km³/s² | Brown & Schaller (2007), *Science* 316, "The mass of dwarf planet Eris" — measured from Dysnomia's orbit |
| Haumea | 267 km³/s² | System mass 4.006×10²¹ kg → GM_sys ≈ 267.4 km³/s² | Ragozzine & Brown (2009), *AJ* 137, mutual-orbit solution for Hiʻiaka + Namaka |
| Pluto | 869.6 km³/s² | GM_Pluto = 869.6 km³/s² (Pluto **alone**; GM_Charon ≈ 105.9) | Brozović et al. (2015), *Icarus* 246, "The orbits and masses of satellites of Pluto"; a post-New-Horizons update exists (Brozović & Jacobson 2024, *AJ* 167:256) if you want the freshest digits |

**The hidden decision.** Eris's 1108 and Haumea's 267 are *system* GMs —
and since both were measured *from their moons' orbits*, the system value
is exactly the right μ for propagating Dysnomia/Hiʻiaka/Namaka. Pluto's
869.6, by contrast, is Pluto-only, and Charon is 12% of Pluto's mass.
`kepler` reconstructs mean motion from μ = parent GM when no fitted
override exists (moons have none), so with 869.6 Charon's period comes
out ≈ 6.77 d instead of the true 6.387 d — a ~6% error that also shifts
Nix and Hydra slightly.

Options:
- **(a) Recommended:** store `pluto.gm = 975.5 km³/s²` (Pluto + Charon,
  869.6 + 105.9), with the `source` string saying exactly that. Best
  two-body fit for *all* Pluto-system moons, consistent in spirit with
  the Eris/Haumea system values, and invisible everywhere else (Pluto's
  own heliocentric orbit comes from SBDB, not from its GM).
- (b) Keep 869.6 and fold the ~6% Charon-period error into the existing
  "Charon circles Pluto" declared simplification (ARCHITECTURE §6).

Either way, the `source` field should carry the citation and the
system-vs-body choice explicitly, so the number is auditable.

**Radii `TODO(review)` process suggestion:** have an agent build a table
of all 66 radii against Horizons `OBJ_DATA` / JPL SSD physical-parameter
pages plus Archinal et al. (2018, IAU WG report) for planets and major
moons, flag any manifest value off by > 2%, and hand you the diff. You
sign; the agent commits.

---

## Q3 — 3I/ATLAS nucleus radius

Current literature (all radius, not diameter):

- **HST imaging constraint:** nucleus diameter between 0.32 and 5.6 km,
  i.e. radius ≈ 0.16–2.8 km at geometric albedo p_V ≈ 0.04 (NASA 3I/ATLAS
  page, observations through Aug 2025; confirmed independently by
  ground-based PSF-decomposition work, arXiv:2512.22365).
- **Non-gravitational-acceleration estimates:** radius ≈ 0.26–0.37 km
  (dynamical estimate cited in arXiv:2512.22365); post-perihelion
  analyses combining NGA with mass-loss rates land around
  radius ~0.4–1.2 km depending on the driving volatile.
- **Post-perihelion HST photometry** (arXiv:2601.21569, Feb 2026) gives
  nucleus photometry and radii under the same p_V = 0.04 assumption,
  consistent with the above bracket.

There is no resolved measurement and won't be; every value is
model-dependent through the assumed albedo.

**Recommendation:** ship **R = 0.5 km**, with a source string along the
lines of: `"nucleus radius adopted 0.5 km; HST constraint 0.16–2.8 km at
pV=0.04 (NASA/HST 2025; arXiv:2512.22365); NGA-based estimates ~0.3 km"`.
Rationale: inside every published constraint, closest to the
dynamically-derived values, and at simulator scale the choice is
invisible anyway — what matters is that the number is cited and inside
the literature bracket. Reasonable alternates if you prefer: 0.3 km
(NGA-anchored) or 2.8 km (upper-limit-anchored).

---

## Q4 — Constellation-figure line set licensing (fast-follow)

Options, worst to best for a proprietary Steam build:

1. **Sky & Telescope / IAU chart figures** — copyrighted chart artwork;
   the *idea* of a figure isn't protectable but their specific line sets
   are best avoided. No.
2. **Stellarium skyculture line sets** — the data files ship under GPL
   with the program; using them in a proprietary binary invites exactly
   the licensing-audit argument WP17 exists to avoid. No.
3. **d3-celestial (Olaf Frohn) data** — BSD-3 code, but the constellation
   line data's provenance is mixed and would need per-file verification
   before shipping. Maybe, with legwork.
4. **In-house authored set over the Yale Bright Star Catalog** —
   recommended. The BSC is public domain and already the WP13 starfield
   source; a line set is just an ordered list of BSC/HR star-number
   pairs per constellation. Authoring the 40-ish culturally standard
   figures (or all 88) from any reference *depiction* while choosing our
   own segments is a few evenings of content work, zero license risk,
   and matches the ARCHITECTURE §10.5 "license-clean line set" language.

**Recommendation:** option 4; keep it a fast-follow as planned. If you
want to accelerate, an agent can draft the HR-pair line lists for the 20
most recognizable constellations for your review.

---

## Q5 — Online capture failed at Jupiter (`no $$SOE in Horizons result`)

### Symptom

`gen-catalog --online` on 2026-07-12: Mercury, Venus, Earth, Mars fetched
and parsed fine (13-epoch TLIST each, identical URL shape), then Jupiter's
response contained no `$$SOE` marker — meaning Horizons returned a
message, not an ephemeris — and the run aborted.

### Diagnosis

The only per-body difference on the planet route is `COMMAND`. The
Horizons manual documents the mechanism that separates Mars from Jupiter
here: planet-**center** ephemerides (599, 699, 799, 899) are defined by
each system's *satellite solution* and are therefore available only over
the satellite solution's limited time span, while planetary-system
**barycenters** (IDs 1–9) come from the DE planetary integration and are
available over roughly ±9999 years. Our planet TLIST spans 1800–2300;
the first body whose center-span doesn't cover the request is exactly
where the run dies — and that is the first giant planet. When a requested
time falls outside a target's span, Horizons returns an explanatory
message in `result` with no `$$SOE`, which is precisely what the parser
reported.

The manual additionally recommends requesting *barycenters* when
generating osculating-element output, because planet-center elements
carry the wobble of the planet about its own system barycenter aliased
into the elements — noise for a fit like ours.

Confidence check completed 2026-07-13 at the failing boundary. Jupiter
center (`COMMAND='599'`) at the generator's JD 2561120 sample reports no
ephemeris after 2200, while Jupiter system barycenter (`COMMAND='5'`)
returns a valid `$$SOE` ELEMENTS record. The earlier 1800 probe also
returns center data, as expected; it did not exercise the upper end of
the 1800–2300 TLIST. Raw-response capture now preserves this diagnostic
payload automatically.

### Proposed fix (needs your sign-off — curated route + §5.3 wording)

1. **Giant-planet routes switch to system barycenters:**
   `599→5, 699→6, 799→7, 899→8`. `CENTER='500@10'` unchanged — heliocentric
   elements remain the right frame for a sim that puts the Sun at the
   parent origin. Mercury–Mars keep 199–499: they demonstrably work, and
   minimal change is the house rule.
   - Positional cost: a giant planet's offset from its own system
     barycenter is set by its moons — order 100 km for Jupiter
     (Callisto-dominated), similar or smaller elsewhere. Far below any
     per-category two-body tolerance in the spot-check budget, and the
     fitted elements actually get *smoother* (no satellite-wobble
     aliasing), which is a small accuracy win for the secular fit.
2. **Diagnostics hardening (non-controversial, land regardless):** on
   parse failure, dump the raw response to
   `target/xtask-debug/<id>.response.txt` and cite the path in the error.
3. **`--capture DIR`:** the online fetcher writes every raw response to
   disk; the directory gets committed — this is the "captured API
   responses for reproducibility" the WP3 checklist already demands, so
   it's a requirement being implemented, not new scope.

### Touched surfaces

`xtask/src/manifest.rs` (four route strings + a route test),
`xtask/src/fetch.rs`/`lib.rs`/`main.rs` (capture + dump),
`docs/wp3-gen-catalog-spec.md` (route table), dry-run output (cosmetic),
and **ARCHITECTURE §5.3's route wording — your edit**, since agents may
not touch that file.

**Recommendation:** approve 1–3; an agent lands them in one change with
the manifest route test, then Part B of the setup guide runs the real
capture.
