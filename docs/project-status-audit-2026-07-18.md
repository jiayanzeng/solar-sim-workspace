# Project status audit — 2026-07-18

**Snapshot date:** 2026-07-18 (Asia/Shanghai)

**Audited branch:** `codex/ui-gameplay-remediation`

**Audited source commit:** `2b145af0e869a77ce0a26b66a8659c4af315f828`
(`docs: correct settings serialization format`)

**Purpose:** Preserve a repository-wide status snapshot for later human audit.

**Scope:** Source, architecture/status documents, tests, offline data/assets,
CI configuration and current public GitHub metadata. No application source,
dependency, generated catalog, or truth fixture was changed during this audit.

## 1. Executive conclusion

The project has a healthy, well-tested local source baseline but is **not yet a
Steam-beta-complete product**.

- Work packages 0–15 are recorded complete. The architecture-conformance and
  UI/gameplay corrective cycles through WP14 have also been closed with tests.
- WP16 is currently **deferred**, with only its default-build Steamworks
  isolation acceptance item checked. The macOS overlay retest, Windows overlay
  test, signing/notarization/depot pipeline, two-platform Steam install, and
  bundle-size evidence remain incomplete.
- WP17 is **todo**. Its end-to-end replay/demo, real-reference-hardware
  performance, both-OS release evidence, and signed licensing audit are not
  complete.
- WP18 remains intentionally deferred.
- The audited source passes the full local workspace, optional Steam feature,
  format, clippy, texture-metadata, and catalog-plan gates.
- The current feature branch has no pull request, no GitHub check runs, and no
  hosted CI result for its head commit. It is 32 commits ahead of `main` and
  zero behind. The local gates are therefore strong evidence, but they do not
  replace a current macOS/Windows hosted run.
- Several user-facing UI/gameplay requests remain unimplemented or require new
  product rulings. They are catalogued separately in
  `docs/ui-gameplay-request-architecture-review-2026-07-18.md`.
- Project documentation has three material consistency defects: the README
  says WP16 is in progress while the authoritative dashboard says deferred;
  the README and dashboard headline still say 223 tests while the current
  suite has 345; and the README says CI runs on every push/PR even though
  branch pushes run CI only on `main`.

Accordingly, the correct status is: **feature-complete through the WP15 and
corrective-cycle scope, locally green, but not integrated into `main`, not
currently hosted-CI-certified at branch head, and not release-ready.**

## 2. Sources and method

The audit read and cross-referenced:

- the complete read-only `ARCHITECTURE.md` Rev C;
- `TASKS.md`, including its dashboard, work-package briefs, Open questions,
  Next up list, and newest-first change log;
- all project audit/stabilization and WP16 documents under `docs/`;
- workspace manifests, all 45 Rust source files, both GitHub workflow files,
  asset/provenance sidecars, and the generated catalog in read-only mode;
- the working-tree state, branch history, branch divergence, remote branch,
  held stash, and public GitHub branch/actions/PR/release metadata;
- fresh local verification commands listed in §6.

Static debt search found no `TODO`, `FIXME`, `todo!`, `unimplemented!`, or
`TODO(review)` marker in Rust source. The matches that remain are historical or
instructional references in `ARCHITECTURE.md`, `AGENTS.md`, `TASKS.md`, and
audit documents.

## 3. Work-package status

`TASKS.md` is authoritative. The current dashboard resolves to:

| Work packages | Current status | Audit interpretation |
|---|---|---|
| WP0–WP15 | ✅ done | Core, catalog, Bevy application, camera, orbit rendering, HUD, settings/recovery, textures and goldens are accepted within their defined scopes. |
| WP16 | deferred | Partial Steam adapter/tooling exists, but the release-engineering package is not complete and must not be resumed implicitly. |
| WP17 | todo | Release QA gates have not been executed/accepted as a package. |
| WP18 | deferred | Optional Compare Size mode remains outside beta scope. |

This is 16 completed work packages, one todo package, and two deferred
packages; those counts must not be interpreted as a completion percentage
because the remaining release packages contain hardware, signing, Steam, and
cross-platform gates of very different size.

### 3.1 Completed corrective cycles

The most recent change-log evidence records:

- architecture conformance AC-1 through AC-3 complete: Layers panel state is
  command/replay-owned, the Rev C plugin graph and frame ownership are restored,
  and Search precedes Menu visually and semantically;
- UI/gameplay UA-1 through UA-6 complete: camera-dolly continuity, ordinary-HUD
  pointer ownership, non-blocking responsive toasts, fixed-epoch convergence,
  and synchronous native OOM notification;
- Q15 compatibility recovery complete: explicit Settings/CLI reset plus a
  transient minimum visual-cue recovery surface that preserves exact saved
  layer values;
- Q16 preserved: Sun and planets remain text-only, Saturn has no reticle, and
  Io provides Icons-layer reticle blend coverage;
- Q17 complete: native macOS/Windows critical-error surfaces run synchronously
  before `StopRendering`, without adding a dependency.

The latest accepted corrective-cycle source evidence is 345 default tests and
242 Steam-feature tests. The new twelve-item request reviewed on 2026-07-18 is
not part of that closed cycle and has not been implemented.

## 4. Architecture and implementation health

### 4.1 Strongly evidenced boundaries

The following architecture boundaries have direct source and test coverage:

- `sim-core` remains engine-agnostic and has only `serde` and `ron` runtime
  dependencies;
- default application/tooling builds are offline; the HTTP dependency is
  optional and confined to `xtask --features online`;
- UI/pointer/keyboard behavior is reduced through `SimCommand`, with replay
  input, corrupt-input rejection, and deterministic state-hash tests;
- all 66 body states remain f64 heliocentric kilometres; moon states compose
  parent-relative truth and the only render mapping rebases around the f64
  camera focus;
- retained orbit paths use the same current orbital elements and parent GM as
  propagation, with elliptic/hyperbolic and cache-reuse coverage;
- transition-only range/extrapolation/emphasis notifications are tested;
- settings normalization, persistent reset, render recovery, UI focus,
  responsive reachability, AccessKit labels, and the Q15/Q16/Q17 rulings have
  explicit regressions;
- default builds exclude Steamworks, while the optional Steam adapter is
  confined behind `PlatformServices` and the `steam` feature.

### 4.2 Current UI/gameplay audit findings

The companion report records twelve requests in four classes:

- implementation defects/gaps: camera reset/left-drag discoverability and the
  apparent moon-reticle drift;
- compatible enhancements: selected-system/orbit highlighting, orbit picking,
  input aliases, Help, centered time dock, and code-defined rail icons;
- content debt: 66-body description completion;
- architecture/product decisions: non-Sun/adaptive body scaling and comet
  tails, +1 day/s default startup, removal of the Moons layer, and deterministic
  region-preset semantics.

No source work should begin from that list until its integrated phase is
selected in `TASKS.md`. Items that conflict with Rev C require a human ruling
rather than an implementation guess.

### 4.3 Documented simplifications that are not violations

The project intentionally models two-body motion, parent-centric moons,
spherical Haumea, no comet nongravitational forces, a fixed visual-grade
TT−UTC offset, and no orbital period for hyperbolic bodies. These are declared
architecture choices. The deferred “Hyperbolic orbital-period omission —
justified” item is documentation clarification only; current source behavior
is tested and is not an implementation violation.

## 5. Repository and data inventory

| Area | Audited state |
|---|---|
| Workspace | 3 members: `sim-core`, `solar-sim`, `xtask` |
| Rust source | 45 `.rs` files |
| Test baseline | 345 default workspace tests: 53 `sim-core`, 241 `solar-sim`, 48 `xtask` library, 2 xtask smoke, 1 active spot-check |
| Optional Steam suite | 242 `solar-sim` tests |
| Catalog | Frozen 66 bodies: 1 star, 8 planets, 9 dwarf planets, 8 asteroids, 32 moons, 8 comets |
| Captured generation inputs | 68 files under `xtask/fixtures/captured-2026-07` |
| Position truth | Active 10-body catalog-epoch gate plus the approved Halley historical gate; 20 captured records retained for audit |
| Textures | 16 KTX2 assets and 16 byte-exact public-domain license/source sidecars |
| Starfield | 5,000-point NASA HEASARC BSC5P-derived baked asset with provenance |
| Catalog descriptions | 66 fields total; 45 empty; 21 mostly one-sentence entries |
| Asset footprint | Approximately 92 MB in `assets/` in this checkout |

The catalog composition tests, manifest ordering, parent GM validation, and
generated/truth-file rules remain intact. The 45 missing descriptions are
non-blocking lints today but conflict with the schema's intended 2–4 sentence
Info content and should be treated as a future curated-content phase.

## 6. Fresh local verification

All commands below were run from the repository root against source commit
`2b145af` before the documentation-only files from this audit were added.

| Command | Result | What it establishes |
|---|---|---|
| `cargo test` | PASS — 345 tests, 0 failed/ignored | Full default workspace baseline, including active spot-check |
| `cargo test -p solar-sim --features steam` | PASS — 242 tests | Optional Steam adapter build/test path |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS | Default workspace has zero clippy warnings |
| `cargo clippy -p solar-sim --all-targets --features steam -- -D warnings` | PASS | Steam feature has zero clippy warnings |
| `cargo fmt --all -- --check` | PASS | Rust formatting is clean |
| `scripts/check-texture-metadata.sh` | PASS — 16 assets | Texture sidecars and exact bytes are complete |
| `cargo run -p xtask -- gen-catalog --dry-run` | PASS — 66-body fetch plan | Offline generator route/manifest plan is valid without network |
| `git diff --check` | PASS | Pre-report checkout had no whitespace errors |

### 6.1 Evidence not refreshed by this audit

This documentation audit did **not** claim or rerun:

- an interactive visual walkthrough of all twelve newly reported behaviors;
- a release-mode Metal smoke/nonblack run at the current branch head;
- a real-GPU DX12 launch or golden capture;
- a real Steam overlay session on either operating system;
- signing, notarization, stapling, SteamPipe install, or bundle measurement;
- WP17 M1 MacBook Air / GTX 1650-class performance measurements;
- a current hosted macOS/Windows CI run for `2b145af`.

Historical evidence for earlier accepted commits remains in `TASKS.md`, but it
must not be represented as current-head evidence for the 32-commit branch
delta.

## 7. Git and hosted CI status

### 7.1 Local/remote branch state before report creation

- Local branch and public remote branch both pointed at `2b145af0`.
- The worktree was clean.
- The branch is 32 commits ahead of public `main` (`374f6905`) and zero behind,
  changing 34 files in the local comparison.
- A held stash remains isolated and untouched:
  `stash@{0}: On codex/wp16-steam-adapter: hold-wp16-steam-msaa-2026-07-16`.
- The head commit is unsigned according to public GitHub metadata.

After this audit is written, the only expected working-tree changes are these
two report files and the append-only `TASKS.md` audit record. No source change
is part of this documentation package.

### 7.2 Public GitHub state checked on 2026-07-18

- The [feature branch](https://github.com/jiayanzeng/solar-sim-workspace/tree/codex/ui-gameplay-remediation)
  exists publicly at `2b145af0` and is not protected.
- GitHub reports no pull request for the branch, zero check runs/status
  contexts for its head, and zero branch-scoped Actions runs.
- The [branch comparison](https://github.com/jiayanzeng/solar-sim-workspace/compare/main...codex/ui-gameplay-remediation)
  reports the branch ahead of `main` by 32 commits and behind by zero.
- The most recent `main` CI run is
  [run 29464166636](https://github.com/jiayanzeng/solar-sim-workspace/actions/runs/29464166636),
  completed successfully on 2026-07-16 for `374f6905`. It predates the entire
  current feature-branch delta.
- No GitHub release exists.

### 7.3 Workflow coverage and process gap

`.github/workflows/ci.yml` runs on pushes to `main` and on pull requests. It
does **not** run on an ordinary push to this feature branch. Because there is no
pull request, the branch head receives no lint/Linux/macOS/Windows/invariant
jobs. `.github/workflows/goldens.yml` is manual-dispatch only.

This creates a concrete integration risk: local macOS results are green, but
the branch cannot satisfy its cross-platform submission standard until a pull
request or another explicitly authorized hosted run exercises the branch head.
Branch protection also does not currently require those checks before merge.

## 8. Release-readiness gaps

### 8.1 WP16 — deferred and incomplete

Completed groundwork:

- optional `steamworks = 0.13.1` feature and no-op/default platform boundary;
- Steam callback/status tests and default-build dependency isolation;
- interim App ID 480 pin and hard package/depot preflight refusal;
- macOS development entitlement/signing preparation tooling.

Still required:

- corrected macOS real-client overlay retest (the initial result failed and is
  explicitly not acceptance evidence);
- Windows/DX12 real-client overlay test;
- real Solar Sim App ID and partner-account approval;
- Apple Developer ID and protected Steam/signing secrets;
- sign/notarize/staple dry-run, Windows signed package, SteamPipe depots;
- dev-branch installation launch on both operating systems;
- measured bundle size no greater than 150 MB per platform.

### 8.2 WP17 — todo

Still required:

- unattended end-to-end demo script on both operating systems;
- recorded replay library with state-hash assertions and no skipped sessions;
- all-layers ≥60 fps evidence on the specified M1 MacBook Air and GTX
  1650-class laptop, or a human-approved architecture/brief amendment;
- exact Metal and DX12 real-hardware smoke gates;
- completed, human-signed licensing audit for fonts, textures, star data, and
  no NASA branding.

### 8.3 Human/external blockers

Open Q13 contains the principal release blocker: hosted-only CI cannot replace
the required physical Windows/Steam/reference-hardware evidence. It also
requires a purchasing/reference-machine decision before packaging. Real Steam
and Apple credentials must be provisioned through protected environments.

## 9. Open questions and deferred decisions

| Item | Status and impact |
|---|---|
| Q4 | Open; constellation-figure line-set licensing, fast-follow rather than current beta core. |
| Q12 | Open; CI-1 through CI-6 are named but have no authoritative briefs, so agents must not infer their scope. |
| Q13 | Open; Windows/Steam/reference-hardware and credential strategy blocks WP16/WP17 acceptance. |
| Hyperbolic period wording | Deferred documentation clarification only; source omission is justified and tested. |
| New UI/gameplay architecture rulings | Initial-body visual proxy/tails, +1 day/s startup, Moons-layer removal, and region-preset semantics await review of the companion report. |

Q14 through Q17 are closed and implemented/documented within their approved
scope. None of the new requests should silently reopen those rulings.

## 10. Documentation consistency findings

These are audit findings, not silent corrections, because `TASKS.md` has a
binding append-only/update protocol and the README is not the status authority.

1. **Stale test headline.** `TASKS.md` and `README.md` still show 223 tests
   (53 + 119 + 48 + 2 + 1). The newest change log and fresh run show 345
   (53 + 241 + 48 + 2 + 1).
2. **Conflicting WP16 status.** `TASKS.md` dashboard says `deferred`; README
   headline/table say `in progress`. `TASKS.md` wins.
3. **Inaccurate CI trigger description.** README says CI runs on every
   push/PR. The workflow says pushes only to `main`, plus all pull requests.
4. **Submission evidence fragmentation.** Historical hosted runs are recorded
   for earlier commits, but the current branch head has no hosted checks or PR.
   The project must distinguish “local current-head green” from “historically
   hosted green.”

Recommended documentation maintenance, when authorized, is a small dedicated
documentation package that updates README status/test/trigger text and uses an
allowed `TASKS.md` protocol change to reconcile the headline baseline with the
already-recorded 345-test evidence.

## 11. Recommended next decisions

1. Review and rule on the architecture-conflicting items in the companion
   UI/gameplay report before starting another source phase.
2. Select exactly one compatible UI/gameplay phase and record it as the sole
   active work package; the safest first phase is all-system reticle/projection
   integrity because it addresses an apparent correctness defect without
   changing physics.
3. Decide whether to open a pull request for the existing 32-commit branch so
   the current head receives the required hosted Linux/macOS/Windows/invariant
   checks before any merge.
4. Keep WP16 deferred and its stash isolated until the human explicitly resumes
   it and Q13's hardware/credential prerequisites are satisfied.
5. Schedule a separate documentation-consistency cleanup; do not mix it into a
   gameplay source phase.

## 12. Audit boundary

This report is a point-in-time record, not an acceptance or release
certificate. A passing local suite does not establish visual correctness on
every GPU, Steam overlay injection, packaging/signing, real-hardware
performance, or the newly requested interaction behavior. It also does not
authorize a merge, pull request, commit, push, architecture edit, dependency
change, or resumption of deferred WP16 work.
