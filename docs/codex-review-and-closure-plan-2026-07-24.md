# Review of Codex's closure conclusions + next-step plan (2026-07-24)

**Scope.** Independent verification of the three findings in Codex's playability-closure
report against the repomix source bundle, plus the resulting task plan. Source of truth for
verification: the bundled tree (`AGENTS.md`, `ARCHITECTURE.md`, `TASKS.md`,
`docs/decision-record-2026-07-22.md`, `crates/**`). Nothing in this document changes source,
closes an Open question, or edits a protected file.

---

## 0. Verdict in one paragraph

Codex's three findings are correct and I reproduced all three from source. Its task plan is
sound and I would execute items 1–5 substantially as written, with one changed
recommendation (the `sim-core` remediation route) and one sequencing correction. I found two
additional defects in the decision ledger that Codex missed, both in
`docs/decision-record-2026-07-22.md`. The important strategic point is that **no software
work is on the critical path any more**: Q28 is roughly thirty minutes of mechanical change,
and everything else that is unchecked is gated on hardware that D4 authorized but that has
not been bought. The single highest-value next action is executing the D4 purchase, not
writing code.

---

## 1. Verification of Codex's findings

### F1 — Q28 redundant schedule membership: **confirmed**

`ModalSurfaceSet::Rebuild` and `::Focus` are configured as a chain nested inside
`SimulationSet::Render` (`input_intent.rs:311-316`). Three systems then also carry `Render`
membership directly, producing the redundant hierarchy edge:

| Site | Mechanism |
|---|---|
| `help.rs:111-113` | `rebuild_help_modal.in_set(ModalSurfaceSet::Rebuild).in_set(SimulationSet::Render)` — explicit double membership. |
| `search.rs:622-630` | `rebuild_browse_menu.in_set(ModalSurfaceSet::Rebuild)` inside a tuple that carries `.in_set(SimulationSet::Render)`; tuple membership distributes to every member. |
| `settings.rs:809-819` | Same tuple-distribution pattern for `rebuild_settings_screen`. |

Exhaustive check: `ModalSurfaceSet` appears at exactly ten sites repo-wide. The only other
consumers are `layers.rs:482-483` (`.after(Rebuild).before(Focus)` — ordering edges, not
membership, so not redundant) and `input_intent.rs:335` (`reconcile_modal_focus` in `Focus`
only, which is the correct pattern and the model for the fix). **Three sites, not more.**

Codex's characterisation is right on both counts: this is not a simulation-architecture
violation, and it changes no behaviour. `Rebuild ⊂ Render`, so removing the direct `Render`
membership leaves every system in the same effective set and every ordering edge intact.

### F2 — `sim-core` agent-rule conflict: **confirmed, and it is exhaustive**

I audited the whole crate rather than the cited lines. `sim-core` contains:

- **Exactly three lines mentioning `f32`**, all in `time.rs:175-180`: `slider_pos(self) -> f32`,
  its body, and `from_slider_pos(p: f32)`. Nested rule 5 ("f64 everywhere. No f32 in this
  crate") is violated only here.
- **Exactly one `unwrap()`/`expect()` outside a `#[cfg(test)]` module**: `time.rs:191`, the tail
  of `from_slider_pos`. Nested rule 4 and root style rule both prohibit it. Test modules begin
  at `catalog.rs:749`, `kepler.rs:343`, `time.rs:577`; every other occurrence is inside one.
- No `std::fs`, `std::env`, `SystemTime`, `std::net`, or RNG. Dependencies remain frozen at
  `serde` + `ron`. Rules 1–3 are clean.

Two details Codex did not surface that change the shape of the fix:

1. **The `unwrap()` is unreachable.** `(p.clamp(-1.0, 1.0) * 12.0).round() as i8` yields
   `i ∈ [-12, 12]`, and the zero case substitutes ±1, so `RateIndex::new` always returns
   `Some`. This is a style violation, not a latent panic — which is why it survived to now
   and why it carries no urgency.
2. **The public slider API is redundant with the already-public `RateIndex::new` / `get`.**
   The sole caller is `time_bar.rs:157-168`, and it has *already* clamped and rounded to an
   integer detent and short-circuited zero before it calls into `sim-core`. See §3.2.

### F3 — Q13 / D4 ledger divergence: **confirmed, with a refinement**

`docs/decision-record-2026-07-22.md:45` (D4) rules "acquire the WP17 reference hardware as
written rather than amend the brief" and names both target machines. `TASKS.md:782` (Q13)
still ends at the 2026-07-16 partial ruling and never mentions D4. Grep of `TASKS.md` for
D-numbers: D1 ✓, D2 ✓, D3 ✓, **D4 ✗ (zero references)**, D5 ✓, **D6 ✗ (zero references)**,
D7 ✓, D8 ✓.

**Refinement.** Codex proposes transcribing D4 and leaving Q13 open for credentials. Correct,
but the transcription must distinguish two states that "open" currently conflates:

- The hardware half is **decided and unexecuted** — D4 settled *what to buy*; the machines do
  not exist yet.
- The credential half is **undecided** — partner account, real App ID, Apple Developer ID,
  protected environments.

Writing this as one undifferentiated "open" hides the fact that the hardware half now needs a
*purchase*, not a *decision*, and that purchase is the longest-lead item in the program.

---

## 2. Findings Codex missed

### N1 — Duplicate `D5` heading in the decision record, both actively cited

`docs/decision-record-2026-07-22.md` contains two sections numbered D5:

- line 70 — "D5 — Q18 amended: primary-surface availability conjunct"
- line 121 — "D5 — Final-stage on-site Windows test plan (hardware TBD accepted)"

`TASKS.md` cites "D5" twenty-four times and means **both**: lines 945–1013 refer to the Q18
amendment; line 1618 ("the WP15 operator procedure now uses D5's exact real-GPU command")
refers to the Windows test plan. A decision ledger whose identifiers do not resolve uniquely
is a correctness problem for exactly the audience it exists to serve — a future agent
resolving "per D5" will pick one of two unrelated rulings. Recommend renumbering the
second occurrence to **D9** and adding a one-line "formerly the second D5" note, then fixing
`TASKS.md:1618`. This is a human-authorised documentation edit, not agent-initiated.

### N2 — D6 has an untracked future trigger

D6 (license/public-repo posture) is not referenced in `TASKS.md` at all. It records that the
proprietary `LICENSE` gap is resolved — confirmed, `LICENSE` exists at root — and then defers
a decision: an explicit go/no-go on privatising the repository, with paid macOS CI minutes
budgeted, **at the 1.0 release decision**. That is a real future commitment with a real cost
attached and it currently lives only in a docs file nobody is required to reread. It belongs
on the status board.

### N3 — The "accepted framework noise" option has a trap variant

If Q28 is ruled "accept the diagnostic," the obvious implementation is to set
`hierarchy_detection: LogLevel::Ignore` on the app's schedules. That would suppress the
diagnostic **globally and permanently**, hiding every future redundant edge, including ones
that would indicate a genuine ordering mistake introduced on unfamiliar hardware during
WP16/WP17 bring-up. If the human rules "accept," the ruling text must say *documented in
`docs/`, detection left enabled* and explicitly prohibit the suppression route.

### N4 — Closed gap, recorded for completeness

The LICENSE-at-root gap tracked in earlier reviews is resolved (D6, and the file is present).
No action.

---

## 3. Recommended rulings

### 3.1 Q28 → **mechanical cleanup**

Recommend the split, not the acceptance. Reasons, in order of weight:

1. **Cost asymmetry.** The fix is three registration edits and one regression test. The
   acceptance option costs a `docs/` page plus a permanent warning on every launch.
2. **Provable behaviour neutrality.** Because `Rebuild` is nested in `Render`, every affected
   system stays in the same effective set. The only thing that must be preserved is the
   intra-plugin `.chain()` ordering, which converts to two explicit `.after()` edges. The
   cross-plugin edges (`layers.rs`'s `.after(Rebuild).before(Focus)`,
   `ensure_focused_control_visible.after(Focus)`) key off set membership, which does not change.
3. **Diagnostic hygiene before hardware bring-up.** WP16/WP17 put the app on two machines
   nobody has run it on. The schedule diagnostic channel is part of the early-warning system
   for that phase, and a channel with a known permanent false positive gets ignored.
4. **It is a shipping commercial product.** A clean launch log is cheap polish.

**Owner.** The change crosses `UiKit`→`HelpPlugin`, `SearchMenuPlugin`, and
`SettingsUiPlugin`, so it needs a coordinating work package. Precedent: AC-2
("plugin-graph non-compliance … §8.2 plugin names, responsibilities, and frame ownership")
was coordinated under **WP4**, which owns the frame schedule. Recommend WP4 again, with the
UIO-7 closeout entry recorded under WP17 afterwards.

### 3.2 New Q29 (`sim-core` f32 + `unwrap`) → **remove the slider API, do not retype it**

Codex recommends retyping `slider_pos`/`from_slider_pos` to `f64` and doing the narrowing at
the Bevy widget boundary. That works. I recommend a different route, because both are
equally breaking changes to a frozen API — so they cost the same single human ruling — and
removal is strictly better on every other axis.

| | A. Retype to f64 (Codex) | B. Remove both methods (recommended) | C. Documented exception |
|---|---|---|---|
| Removes f32 from `sim-core` | yes | yes | no |
| Removes the non-test `unwrap()` | separate edit | yes, structurally | separate edit |
| Frozen-API impact | retype 2 public fns | remove 2 public fns | none |
| Human rulings required | 1 | 1 | 1 |
| New conversion sites introduced | 1 (f64→f32 in widget) | 0 | 0 |
| Public surface after | same size | 2 smaller | same |

Under B, `time_bar.rs` becomes:

```rust
pub fn rate_for_slider_value(value: f32) -> Option<RateIndex> {
    let detent = value.clamp(-SLIDER_LIMIT, SLIDER_LIMIT).round();
    RateIndex::new(detent as i8)          // new() already rejects 0
}

pub fn slider_value_for_rate(rate: RateIndex) -> f32 {
    f32::from(rate.get())
}
```

**Behaviour equivalence is provable, not assumed.** I evaluated both directions over all 24
detents in IEEE-754:

- Current: `(i as f32 / 12.0) * 12.0` is **bit-identical to `i as f32` for every i ∈ [-12,12]\{0}**,
  and the derived layout percentage at `time_bar.rs:862` is bit-identical too. So B changes no
  rendered pixel and cannot move a golden.
- Codex's route A is also exact after the f64→f32 narrowing. So the choice is not a numerical
  one — it is purely about API surface, and B is the smaller surface.
- The reverse direction is exact by construction: the caller already rounds to an integer
  detent, so `from_slider_pos(detent / 12.0)` re-derives the same integer. The
  `p < 0.0` zero-substitution branch inside `sim-core` is dead code on the app path because
  `detent == 0` is short-circuited to `None` first.

The counter-argument for A, stated fairly: the symmetric-log slider mapping is arguably
`sim-core` domain knowledge, and keeping it there means a future non-Bevy frontend inherits
it. That argument was strong when the mapping was non-trivial. It is now the identity map on
integers with a lossy float round-trip wrapped around it, so it carries no knowledge worth
exporting. If the human prefers A on principle, A is safe — it just keeps a layer that earns
nothing.

**This is a P3.** It is pre-existing, CI-green, and unreachable as a panic. It must not be
bundled into the Q28 change (root rule 2, scope discipline) and must not delay UIO-7.

### 3.3 Q13 → transcribe D4, split the ledger, and start the purchase

No new decision needed; D4 already ruled. What is needed is execution. The purchase settles
five unchecked acceptance items across two work packages in one cycle, and the D5 Windows
plan (line 121) is written to run in a single on-site day once the machine exists.

---

## 4. Sequencing

The three streams are independent. Run them in parallel; do not serialise behind Q28.

| Order | Stream | Gate | Owner |
|---|---|---|---|
| **Now** | Execute D4 purchase (M1 Air 8 GB base, GTX 1650-class laptop) | none — already ruled | human |
| **Now** | Provision Steam partner account / real App ID / Apple Developer ID / protected environments | none — long lead time | human |
| **Now** | Block B: ledger reconciliation (D4 + D6 transcription, D5 collision filed) | none — documentation only | Codex |
| After ruling | Block A: Q28 cleanup + regression test + UIO-7 closeout | human closes Q28 | Codex, WP4 |
| After ruling | Block C: `sim-core` remediation | human closes new Q29 | Codex, WP1 |
| On arrival | D5 (line 121) on-site Windows day; WP16 packaging; WP17 gates | hardware + credentials | both |

Codex's plan items 3, 4, and 5 (WP16, WP17, the Q21 conditional trigger) I endorse as
written; they are correctly scoped and correctly parked. Item 5 in particular is right to
resist speculative render-scale work — do not touch the renderer until the M1 Air produces a
number.

---

## 5. Prompt blocks for Codex

Each block is self-contained. **Do not issue Block A or Block C until the corresponding
question is closed in `TASKS.md` by the human.** Block B is safe to issue immediately.

### Block A — Q28 mechanical cleanup (issue only after Q28 is closed "cleanup")

> **Task: remove the three redundant `Rebuild`/`Render` set memberships (Q28), under WP4.**
>
> Q28 has been closed by the human in favour of mechanical cleanup. `ModalSurfaceSet::Rebuild`
> is already nested inside `SimulationSet::Render` by `input_intent.rs`'s `configure_sets`, so
> three systems that carry both memberships produce Bevy's non-fatal hierarchy-redundancy
> diagnostic on every launch. Remove the redundancy without changing a single ordering edge.
>
> Make exactly these four changes.
>
> 1. `crates/solar-sim/src/help.rs`, in `impl Plugin for HelpPlugin` — drop the direct `Render`
>    membership:
>    ```rust
>    app.init_resource::<HelpUiState>()
>        .add_systems(Update, rebuild_help_modal.in_set(ModalSurfaceSet::Rebuild));
>    ```
> 2. `crates/solar-sim/src/search.rs`, in `impl Plugin for SearchMenuPlugin` — split the
>    three-system chain so the rebuild system is in `Rebuild` only, preserving
>    `reset_search_interface → rebuild_search_dropdown → rebuild_browse_menu` with an explicit
>    edge:
>    ```rust
>    .add_systems(
>        Update,
>        (reset_search_interface, rebuild_search_dropdown)
>            .chain()
>            .in_set(SimulationSet::Render),
>    )
>    .add_systems(
>        Update,
>        rebuild_browse_menu
>            .after(rebuild_search_dropdown)
>            .in_set(ModalSurfaceSet::Rebuild),
>    );
>    ```
> 3. `crates/solar-sim/src/settings.rs`, in `impl Plugin for SettingsUiPlugin` — same split,
>    preserving `sync_settings_screen → sync_external_presentation_to_settings →
>    persist_requested_settings → rebuild_settings_screen`:
>    ```rust
>    .add_systems(
>        Update,
>        (
>            sync_settings_screen,
>            sync_external_presentation_to_settings,
>            persist_requested_settings,
>        )
>            .chain()
>            .in_set(SimulationSet::Render),
>    )
>    .add_systems(
>        Update,
>        rebuild_settings_screen
>            .after(persist_requested_settings)
>            .in_set(ModalSurfaceSet::Rebuild),
>    )
>    .add_systems(Update, save_settings_on_window_close);
>    ```
> 4. Add a regression test in `crates/solar-sim/src/lib.rs`'s test module, next to
>    `architecture_plugin_assembly_matches_rev_c_and_owns_helpers_once`. It must fail on
>    reintroduction anywhere in the schedule, not just at these three sites:
>    ```rust
>    #[test]
>    fn update_schedule_builds_without_redundant_set_membership() {
>        use bevy::ecs::schedule::{LogLevel, ScheduleBuildSettings};
>        let mut app = App::new();
>        configure_frame_flow(&mut app);
>        add_architecture_plugins(&mut app);
>        app.configure_schedules(ScheduleBuildSettings {
>            hierarchy_detection: LogLevel::Error,
>            ..default()
>        });
>        let mut schedule = app
>            .world_mut()
>            .resource_mut::<Schedules>()
>            .remove(Update)
>            .expect("Update schedule must exist");
>        schedule
>            .initialize(app.world_mut())
>            .expect("Update schedule must build without redundant hierarchy edges");
>    }
>    ```
>    Leave `ambiguity_detection` at its default (`Ignore`) — this test is about hierarchy
>    redundancy only and must not become an ambiguity gate. If `initialize` trips on a system
>    parameter that requires a resource, narrow the test app to `configure_frame_flow` plus
>    `InputIntentPlugin`, `UiKit`, `SearchMenuPlugin`, and `SettingsUiPlugin` and say so in the
>    change-log entry; do not weaken the assertion.
>
> **Explicitly out of scope.** Do not change `input_intent.rs`'s `configure_sets`. Do not
> touch `layers.rs` (its `.after(Rebuild).before(Focus)` edges are ordering, not membership,
> and are correct). Do not set `hierarchy_detection` on the production app. Do not touch
> `sim-core`. Do not rewrite any historical review document.
>
> **Acceptance.** `cargo test` ≥ 434 default and ≥ 435 with `--features steam` (the new test
> raises the floor; record the exact numbers). `cargo fmt --all -- --check` clean. Debug and
> release `cargo clippy --workspace --all-targets -- -D warnings` clean. One real macOS/Metal
> launch showing the three-edge diagnostic is gone — paste the relevant log lines. Re-run the
> existing golden comparisons for the six canonical views and confirm unchanged ΔE. Then add
> the UIO-7 final closeout entry to `TASKS.md` and check its acceptance box, citing this run.
>
> **Do not** close Q28 yourself — record that the human closed it and cite the ruling.

### Block B — decision-ledger reconciliation (issue now; documentation only)

> **Task: reconcile `TASKS.md` with the 2026-07-22 decision record. No source changes.**
>
> Three transcription gaps exist. `docs/decision-record-2026-07-22.md` states that its
> closures "should be transcribed into `TASKS.md` by the next agent session under the normal
> protocol, citing this record as the human-delegated authorization," so transcription is
> pre-authorised. Closing questions is not.
>
> 1. **D4 → Q13.** Append the D4 ruling to the Q13 entry. Q13's hardware half is now
>    **decided and awaiting execution**, not undecided: D4 rules that both reference machines
>    are acquired as written rather than amending WP17's brief, and names the targets (used
>    GTX 1650-class Windows laptop, 16 GB, 1080p, NVMe, Win 11 Home; base M1 MacBook Air,
>    7-core GPU / 8 GB, explicitly not upgraded). State plainly that no WP16 or WP17 acceptance
>    box is checked by the ruling. Q13 stays **OPEN** for its credential half — Steam partner
>    account, real App ID, Apple Developer ID, protected environments — and for purchase
>    execution evidence.
> 2. **D6 → status board.** D6 is currently referenced nowhere in `TASKS.md`. Record it: the
>    proprietary root `LICENSE` exists and the earlier gap is closed; the repository stays
>    source-visible proprietary through beta; and an explicit go/no-go on privatising, with
>    paid macOS CI minutes budgeted, is due **at the 1.0 release decision**. Add that go/no-go
>    as an unchecked item under **Next up** so it is not lost.
> 3. **Duplicate `D5` — file, do not fix.** `docs/decision-record-2026-07-22.md` has two
>    sections numbered D5: the Q18 primary-surface amendment (line 70) and the final-stage
>    on-site Windows test plan (line 121). `TASKS.md` cites both under the same label —
>    lines 945–1013 mean the first, line 1618 means the second. Open this as **Q30** for the
>    human: renumber the second to D9 with a "formerly the second D5" note and correct the
>    `TASKS.md:1618` citation, or keep the collision and disambiguate every citation. Do not
>    renumber a human authority document yourself.
>
> **Acceptance.** `git diff --check` clean. Append-only change-log entry recording that this
> is documentation-only, that no source, generated asset, work-package status, or test
> baseline changed, and that previously recorded green gates were therefore not re-run. No
> Open question is closed by this task.

### Block C — `sim-core` remediation (issue only after the new Q29 is closed)

> **Task: remove `f32` and the non-test `unwrap()` from `sim-core`, under WP1, per the human's
> Q29 ruling.**
>
> `crates/sim-core/AGENTS.md` rule 5 requires f64 everywhere and rule 4 prohibits
> `unwrap()`/`expect()` outside tests. `time.rs:175-191` violates both, and it is the only
> place in the crate that does — the whole-crate audit found exactly three `f32` lines and
> exactly one non-test `unwrap()`, all here. Rule 6 freezes the public API, which is why this
> needed a ruling.
>
> Apply the ruled option:
>
> - **If the ruling is "remove":** delete `RateIndex::slider_pos` and
>   `RateIndex::from_slider_pos`. In `crates/solar-sim/src/time_bar.rs`, `rate_for_slider_value`
>   becomes `RateIndex::new(value.clamp(-SLIDER_LIMIT, SLIDER_LIMIT).round() as i8)` — `new`
>   already rejects zero, so the explicit zero branch goes away — and `slider_value_for_rate`
>   becomes `f32::from(rate.get())`. This is bit-exact against the current implementation for
>   all 24 detents, including the derived layout percentage at `time_bar.rs:862`, so no golden
>   may move. Move the two `sim-core` slider round-trip tests to `time_bar.rs` as boundary
>   tests and keep 24-detent coverage.
> - **If the ruling is "retype to f64":** change both signatures to `f64`, replace the trailing
>   `.unwrap()` with direct `RateIndex(i)` construction justified by the clamped invariant in a
>   comment, and perform the single `f64 → f32` narrowing in `time_bar.rs`. Keep the existing
>   `sim-core` tests and retype their literals.
>
> **Explicitly out of scope.** Do not touch `kepler.rs` or `catalog.rs`. Do not add a
> dependency. Do not change any other public signature. Do not bundle this with Q28.
>
> **Acceptance.** `grep -rn "f32" crates/sim-core/src/` returns nothing. No `unwrap()` or
> `expect()` outside `#[cfg(test)]` modules in `sim-core`. All 24 detents round-trip. Replay
> state hashes unchanged — run the replay/state-hash tests specifically and say so. Default and
> `--features steam` suites, fmt, and debug/release clippy all green, with the new counts
> recorded. Re-run the six canonical golden comparisons and confirm unchanged ΔE.

---

## 6. Human sign-off checklist

- [x] **Q28 ruling.** Mechanical cleanup selected and completed without globally suppressing
      `hierarchy_detection`.
- [x] **Q28 owner.** WP4 selected as the coordinating package.
- [x] **File Q29** for the `sim-core` slider API. The recommended removal was selected and
      completed after the human-controlled ARCHITECTURE §4.2 amendment.
- [x] **Q30 / D5 collision.** The second D5 was renumbered to D9 with the required
      “formerly the second D5” note, and its `TASKS.md` citation was corrected.
- [x] **Authorise Block B.** The decision-ledger reconciliation is complete.
- [ ] **Execute D4.** Confirm the purchase is proceeding, or state the date it will be
      reconsidered. Procurement of both ruled reference machines is scheduled for
      2026-07-31 and remains pending. Nothing downstream of WP16 moves until these machines
      exist, and no WP16 or WP17 acceptance box is checked by the schedule.
- [ ] **Steam partner account / real App ID / Apple Developer ID / protected environments.**
      Confirm who owns each and the expected date; this is the other long-lead item and it
      gates step 8 of the on-site Windows day. Steam partner onboarding is scheduled for
      2026-07-31 and remains pending; the real App ID follows it. Apple Developer ID and
      protected-environment ownership/dates remain outstanding.
- [x] Confirm that Codex's items 3, 4, and 5 (WP16, WP17, the Q21 conditional render-scale
      trigger) stay parked exactly as written until hardware lands.

---

## 7. Execution status (2026-07-24)

- [x] **Phase 0 — decision-ledger checkpoint.** Block B is complete and the resulting
      documentation changes are committed.
- [x] **Phase 1 — human decisions and architecture gate.** The human selected the first
      recommended option for Q28, Q29, and Q30 and amended ARCHITECTURE §4.2 for Q29.
- [x] **Phase 2 — Block A / Q28.** The redundant schedule memberships were removed under WP4,
      ordering was preserved, and the schedule-wide hierarchy regression, real-Metal smoke,
      tests, lint, and golden comparisons passed.
- [x] **Phase 3 — Block C / Q29.** The public float slider API and non-test `unwrap()` were
      removed from `sim-core`; application-boundary detent mapping and regression coverage
      landed, with replay hashes, tests, lint, and golden comparisons unchanged.
- [ ] **Phase 4 — long-lead external execution.** The D4 hardware purchase and Steam partner
      onboarding are scheduled for 2026-07-31. The real App ID follows onboarding; Apple
      Developer ID and protected-environment owners/dates still require human assignment.
      WP16, WP17, and the Q21 conditional render-scale trigger remain parked until their
      stated prerequisites exist.
