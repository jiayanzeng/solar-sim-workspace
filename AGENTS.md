# AGENTS.md — rules for AI coding agents (repo root)

Applies to the whole repository. Nested `AGENTS.md` files
(`crates/sim-core/`, `xtask/`) add stricter rules for their subtrees and
take precedence there. These files are human-maintained: do not edit any
`AGENTS.md` or `ARCHITECTURE.md`.

## Read first, in this order

1. `ARCHITECTURE.md` — the design of record. **Read-only.** If your task
   seems to require deviating from it, stop and file an Open question in
   `TASKS.md` instead of improvising.
2. `TASKS.md` — what's done, what's next, and the only file you update
   (follow its Update protocol exactly).
3. The relevant `docs/wp*-*.md` spec for your work package, if one exists.

## Commands

```
cargo test                                   # full suite; must be green before and after your change
cargo fmt --all                              # rustfmt defaults; no custom config
cargo clippy --workspace --all-targets      # zero warnings
cargo run -p xtask -- gen-catalog --dry-run  # never hits the network
cargo run -p xtask -- gen-catalog --fixtures xtask/fixtures --allow-partial --out assets/catalog.sample.ron
```

Network builds (`--features online`) are for explicitly network-capable
environments only. Never add `online` to default features or CI.

## Hard rules

1. **Never edit:** `ARCHITECTURE.md`, any `AGENTS.md`, `assets/catalog*.ron`
   (generated; regenerate via `xtask gen-catalog` instead), anything under
   `xtask/fixtures/spotcheck/` (captured truth data).
2. **Scope discipline.** Work on exactly one `TASKS.md` work package at a
   time. Do not "drive-by refactor" unrelated code; if you spot a problem
   outside your WP, record it as an Open question.
3. **No new dependencies without sign-off.** Adding any crate to any
   `Cargo.toml` requires an Open question first, except dev-dependencies
   used only in tests, which you may add but must list in your change-log
   entry. `sim-core`'s dependency set (`serde`, `ron`) is frozen harder —
   see its nested AGENTS.md.
4. **Tests land with code.** Every behavior change ships tests in the same
   change, per ARCHITECTURE §12: invariants + an independent cross-check
   for math/physics; convergence sweeps over the full supported domain for
   solvers; transition-only tests for event emitters; corrupt-input
   rejection for parsers/loaders. The workspace test count in `TASKS.md`
   may only go up (or down with a written justification).
5. **Units and time scales.** Files: km, degrees, JD **TDB**,
   ecliptic-J2000. Runtime: radians, seconds-since-J2000 (TDB). UTC only at
   the wall-clock boundary via `time::t_from_unix_utc`. AU only inside the
   SBDB adapter. If you find yourself converting units anywhere else, you
   are probably in the wrong layer — stop and check ARCHITECTURE §3.5.
6. **Determinism.** No system-clock or RNG reads inside `sim-core` or any
   simulation-state path; wall time is a parameter. All user actions go
   through the `SimCommand` queue — never mutate simulation state from UI
   code directly.
7. **Catalog composition is frozen.** The 66-body list, category counts,
   and manifest ordering tests exist to make silent drift impossible.
   Adding/removing/renaming bodies requires human sign-off via an Open
   question.
8. **Do not weaken a test to make it pass.** If a test fails, the code is
   wrong until proven otherwise; loosening tolerances or deleting
   assertions requires a change-log justification citing the numerical
   reason.

## Style (as practiced in the codebase — match it)

- Module-level `//!` docs state *why* the module exists and which
  architecture section it implements ("WP2 — Rev C §4.3").
- Comments explain reasoning and traps, not restatements of code. Traps
  that were actually hit get a "do not reintroduce" note (see the
  noon-vs-midnight constant in `time.rs`).
- Errors: small enums implementing `Display`; validation collects **all**
  errors, not first-fail. Library code returns `Result`; no `unwrap()`
  outside tests and `main`.
- Test names are sentences: `clamps_at_range_edges_report_once_and_pin`.
- `TODO(review)` marks values awaiting human verification — never delete
  one without the review actually happening.

## Definition of done for any change

- [ ] `cargo test` green (workspace), `cargo fmt` clean, `clippy` zero warnings
- [ ] New behavior covered by tests per rule 4
- [ ] `TASKS.md` updated per its protocol (status + change-log entry with evidence)
- [ ] No edits to read-only files; no unapproved dependencies
- [ ] Doc comments updated where behavior moved

## When uncertain

Prefer stopping over guessing. Ambiguity in the spec, a conflict between
documents, a missing acceptance detail, a value you'd have to invent — all
of these become `TASKS.md → Open questions` entries, and you continue with
whatever part of the task is unambiguous.
