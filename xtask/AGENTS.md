# AGENTS.md — xtask (stricter rules for this subtree)

`xtask` is dev tooling and the repo's only network-capable code. It is
also where the project's data governance lives (ARCHITECTURE §5,
`docs/wp3-gen-catalog-spec.md`). The stakes here are *silent numerical
error* — plausible-looking wrong positions — so the rules are about
provenance and cross-checking, not style.

1. **Network stays behind the `online` cargo feature.** Never add network
   code outside the `Fetch` trait, never enable `online` by default or in
   CI, never add a second HTTP client.
2. **`src/manifest.rs` is the only hand-authored data in the repo.**
   Orbital elements, epochs, rates: always generated, never typed — if a
   number describes motion and you typed it, that's a defect. Curated
   fields (radii, GMs, colors, blurbs) may be edited only with a citation
   in the change, and `TODO(review)` markers are cleared only by an actual
   human review (Open question → sign-off).
3. **Request pinning is schema-level.** The Horizons/SBDB query constants
   (`ELEMENTS`, `ECLIPTIC`, `J2000`, `KM-S`, JD-TDB TLIST, `full-prec`)
   are listed in ARCHITECTURE §5.4. Changing any of them = Open question,
   not a refactor.
4. **Fixtures are labeled.** Synthetic smoke fixtures carry the
   "SYNTHETIC — NOT flight data" marker in their `signature.source`; never
   remove it, never mix synthetic and captured data in one directory.
   Real captures go to `fixtures/spotcheck/` (read-only once committed)
   or replace the synthetic set wholesale in a change that says so.
5. **Emission is gated.** Never bypass `emit::write_catalog`'s validation,
   never write `assets/*.ron` by any other path, never strip the
   provenance header.
6. **Time scales.** Everything in this pipeline is JD **TDB**. If a UTC
   value appears anywhere except the cosmetic `generated_utc` stamp,
   that's the exact class of bug the spec's §5 watchlist exists for —
   stop and check.
7. **Parser changes need adversarial tests.** Any change to
   `horizons.rs`/`sbdb.rs` parsing lands with tests for the malformed
   case, not just the happy path.
