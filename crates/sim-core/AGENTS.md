# AGENTS.md — crates/sim-core (stricter rules for this subtree)

`sim-core` is the engine-agnostic heart shared with the long-term roadmap.
Its purity is a CI-enforced architectural invariant (ARCHITECTURE §3.1),
not a preference.

**Absolute constraints:**

1. **Dependencies are frozen at `serde` + `ron`.** No Bevy or any `bevy*`
   transitive crate, ever. No glam/nalgebra (f64 `[f64; 3]` helpers in
   `kepler` are the vector math). No tokio/async, no chrono (the calendar
   is implemented here deliberately). Proposals to add anything = Open
   question in `TASKS.md`.
2. **No I/O of any kind.** No `std::fs`, no network, no environment reads.
   This crate parses strings and computes; callers own files and sockets.
3. **No clock, no randomness.** Wall time is always a parameter
   (`tick(wall_dt, wall_now_t)`); nothing here calls
   `SystemTime::now()`.
4. **No panics on untrusted input.** Loaders and solvers return `Result`;
   `Catalog::validate()` collects all errors. `unwrap()`/`expect()` only
   in tests.
5. **f64 everywhere.** No f32 in this crate; the f64→f32 rebase is the
   app's floating-origin job.
6. **Public API is a frozen contract** (ARCHITECTURE §4). Additive changes
   fine; renaming/removing/retyping anything public requires an Open
   question — `xtask`, the future app, and the spec doc all bind to it.
7. **Numerical changes need numerical evidence.** Touching a solver,
   tolerance, constant, or conversion requires: the invariant tests still
   pass at their existing tolerances, the convergence sweeps still pass,
   and the RK4 cross-validation still passes. Never tune a tolerance to
   make a change fit.
