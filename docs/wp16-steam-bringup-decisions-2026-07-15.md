# WP16 Steam bring-up — decision brief (2026-07-15)

**Purpose.** Answer Codex's post-merge asks and record the plan for the two
open questions gating WP16: **Q14** (Steamworks dependency + App ID) and
**Q13** (real-hardware strategy). This brief is prepared for the human's
sign-off; per project protocol, only the human closes open questions. Once
signed, the reply block in §3 can be pasted to Codex verbatim.

Repo state this brief is based on: `main` @ `8befd56`, CI run #40 fully
green, test baseline 201, WP16 `in-progress` with the dependency-free
`PlatformServices` boundary, the mock lifecycle test, and the default-build
Steamworks CI guard already landed.

---

## 1. Q14 — recommendation: approve `steamworks = "0.13.1"` with interim App ID 480

### 1.1 The dependency

Approve `steamworks = "0.13.1"` as an **optional dependency of
`crates/solar-sim` only**, behind the cargo feature `steam`. Verified
2026-07-15 against the crates.io API: 0.13.1 is the current
max/newest/default version (last publish 2026-05-05), and its documented
`Client::init_app(AppId)` takes the numeric App ID ARCHITECTURE §11 requires.
The existing CI invariant ("default build excludes Steamworks") already
guards the default tree; it stays as-is and becomes WP16's first acceptance
box.

### 1.2 The App ID: use Valve's public test app (480) now, swap later

**Decision proposed: adopt App ID 480 ("Spacewar") as the interim
development App ID, committed as build metadata, with hard guardrails
(§1.4) so it can never reach a depot build.**

Why this is feasible and standard practice:

- 480 is the App ID of Valve's Steamworks SDK example application. It exists
  precisely so developers can integrate and exercise the Steamworks API
  before (or without) a partner account; every Steam account effectively has
  it. `SteamAPI_Init` / `Client::init_app(480)` works against any logged-in
  Steam client.
- When the app is launched outside Steam (our normal `cargo run` workflow),
  the Steam client identifies the process via a `steam_appid.txt` file next
  to the executable containing the ID. That is the entire dev-loop
  mechanism; shipped builds launched through Steam don't need the file.
- The in-game overlay attaches based on this same identification, so the
  **overlay spike — WP16's top-risk item — runs fully on 480.** No partner
  account is needed to answer "does the overlay work over our
  Bevy/wgpu/Metal swapchain, and does the app behave when it doesn't."
- The rework cost of swapping 480 → real ID later is one committed constant
  plus a regenerated `steam_appid.txt`, provided the guardrails in §1.4 are
  followed. There is no API-shape difference: `init_app` takes the same
  `AppId` either way. The integration work (adapter, lifecycle, callbacks,
  overlay handling) is 100% reusable.

### 1.3 What 480 covers — mapped to WP16 acceptance

| WP16 acceptance item | Covered by 480? |
|---|---|
| Default (non-`steam`) build has no Steamworks in tree (CI) | ✅ already landed; App-ID-independent |
| Overlay spike documented for both OSes; app runs with overlay unavailable | ✅ macOS half runnable **now** on the M2 Pro; Windows half waits on Q13 hardware, not on an App ID |
| Sign/notarize/staple dry-run on macOS | ✅ Apple-side only; needs Developer ID, not Steam |
| `dev`-branch SteamPipe install launches on both OSes | ❌ needs the real App ID (see §1.5) |
| Bundle ≤ 150 MB/platform measured | ✅ App-ID-independent |

So of the four acceptance boxes, only the SteamPipe item is blocked on the
real App ID — and it was already blocked on Q13 hardware for its Windows
half anyway.

What 480 explicitly does **not** provide: SteamPipe depot uploads and
`dev → beta → default` branches, a store presence, our own
achievements/stats/leaderboards (480's belong to Spacewar), and any
ownership/DRM semantics. None of these are in WP16's "init with App ID,
shutdown on exit, nothing else" scope except the SteamPipe item.

One further caution: 480's shared services (lobbies, P2P, Spacewar's
achievement set) are used by thousands of strangers. Our scope calls none of
them — the adapter must stay init/shutdown/overlay-status only, which the
`PlatformServices` trait already enforces structurally.

### 1.4 Guardrails (preserve Q14's "no silent divergence" intent)

Q14's original recommendation was "commit the approved App ID and use no
fallback ID, so local, CI, and SteamPipe builds cannot silently target
different applications." An interim ID is compatible with that intent if and
only if the placeholder is loud and unshippable:

1. **Single source of truth.** One committed constant in the steam adapter
   module, e.g. `pub const STEAM_APP_ID: u32 = 480;` with a provenance
   comment marking it INTERIM (Spacewar / Valve SDK example) and referencing
   this brief. No environment variable, no fallback, no second definition.
2. **Pinned by test.** A unit test asserts the constant, so any change is a
   deliberate, reviewed diff (same pattern as the noon-vs-midnight constant).
3. **Packaging hard-fail.** The WP16 `xtask` packaging/depot scripts MUST
   refuse to run when the App ID is 480 (clear error: "interim dev App ID —
   assign the real App ID before building depots"). This makes shipping the
   placeholder mechanically impossible, which is stronger than a checklist.
4. **Generated, never hand-written.** `steam_appid.txt` for the dev loop is
   emitted by `xtask` (or a dev launch script) from the same constant and is
   gitignored, so it can never disagree with the code.
5. **Swap procedure.** When the real App ID is assigned, an agent files a
   one-line open question ("replace interim 480 with App ID N"), the human
   closes it, and the diff touches exactly the constant + test. The
   packaging guard from (3) is then retargeted to reject 480 forever.

### 1.5 The real App ID — when and how

A real App ID requires a Steamworks partner account (Steam Direct: identity,
tax, and bank verification, plus a per-app fee — US$100 at last check,
recoupable after $1,000 revenue). Onboarding is human-only and has multi-day
verification latency, so it should **start before WP16 packaging begins**
(alongside the Q13 hardware decision and the Apple Developer ID), but it
does not block anything Codex can build now.

---

## 2. Q13 — Mac-first development plan

**Decision proposed: continue Mac-only development now; defer the Windows
hardware decision to the existing deadline ("before WP16 packaging begins").
This confirms the Windows-without-Windows strategy already documented in
`docs/wp0-dev-setup-macos.md` — it does not weaken any acceptance criterion.**

What proceeds fully on the M2 Pro + hosted CI, right now:

- The entire `steam`-feature adapter, its mock tests, and launch commands
  (Codex's next step once Q14 closes).
- The **macOS overlay spike** against App ID 480. Note the environment has
  improved since the risk register was written: the Steam client for macOS
  is now native Apple Silicon (rolled out from the June 2025 beta), so an
  arm64 Steam client injecting into our arm64-native app is the
  representative configuration — no Rosetta asterisk on the spike result.
- Sign/notarize/staple dry-run (once the Apple Developer ID exists).
- Everything Windows that hosted CI already proves: compile, link, full test
  suite, and the WARP smoke code-path check (green in runs #38/#40; ~7 min
  including a 2.5 min WARP smoke).

What genuinely requires real Windows hardware and stays deferred (unchanged
from Q13's own text):

1. WP16 Windows overlay spike (real Steam client, real desktop session).
2. WP16 Windows half of the dev-branch SteamPipe install launch (also needs
   the real App ID, §1.5).
3. WP17 perf gate on the GTX 1650-class reference laptop, plus real-GPU DX12
   golden validation (Q10).

Options at the deadline, unchanged: acquire the hardware (a used GTX
1650-class laptop is the cheap, faithful route — it is the reference machine
by definition) or approve a signed amendment to WP17's reference-machine
criterion. Cloud Windows GPU instances are not a substitute for either the
overlay spike (virtual display sessions aren't representative) or the perf
gate (no credible frame-time measurement), so they are not proposed.

Net effect of §1 + §2 together: **nothing in WP16 is idle.** The interim App
ID unblocks the adapter and the macOS overlay spike immediately; the only
items left waiting are exactly the ones already waiting on hardware and
partner onboarding, both of which have a clear trigger (before packaging).

---

## 3. Reply to Codex (paste after sign-off)

> Q14 closed. Approve `steamworks = "0.13.1"` as an optional dependency of
> `crates/solar-sim` behind the `steam` feature. App ID = **480** (Spacewar,
> Valve's public SDK test app) as an **interim development ID** — commit it
> as build metadata per your recommendation, with these binding guardrails:
> (a) one constant, provenance-commented as INTERIM, no fallback, pinned by a
> unit test; (b) WP16 packaging/depot xtask commands must hard-fail while the
> App ID is 480; (c) `steam_appid.txt` is generated from the constant and
> gitignored, never hand-written. The real App ID will arrive via a new open
> question once the Steamworks partner account exists; swapping is that
> constant plus the regenerated `steam_appid.txt`, nothing else. Full
> rationale: `docs/wp16-steam-bringup-decisions-2026-07-15.md`.
>
> Q13 partial answer: proceed Mac-first. Implement the adapter and give me
> the macOS launch commands; I'll run the real-client overlay check on the
> M2 Pro against 480 and report. The Windows overlay spike, the dev-branch
> install checks, and the WP17 reference hardware remain deferred to the
> existing "before packaging begins" deadline — leave Q13 open for the
> hardware-purchase half. WP16's SteamPipe acceptance box stays unchecked
> until the real App ID and Windows hardware exist; that is expected, not a
> regression.

---

## 4. Sign-off

- [x] Approve §1 (steamworks 0.13.1 + interim App ID 480 + guardrails) — closes Q14
- [x] Approve §2 (Mac-first now; hardware decision deferred to the existing deadline) — narrows Q13, leaves it open
- [x] Paste §3 to Codex; Codex records the closure in `TASKS.md` per the update protocol

Signed: Jiayanzeng  Date: 2026-7-16
