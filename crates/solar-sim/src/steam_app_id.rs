//! WP16 — the single source of truth for Steam application identity.
//!
//! This dependency-free module is also compiled directly by `xtask` so the
//! generated development marker and the application can never diverge.

/// INTERIM: Valve's public Spacewar SDK test application.
///
/// Approved by the human in Q14 for development-only Steam bring-up. See
/// `docs/wp16-steam-bringup-decisions-2026-07-15.md`; release packaging and
/// depot commands permanently reject this value.
pub const STEAM_APP_ID: u32 = 480;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interim_spacewar_app_id_is_pinned_until_human_replacement() {
        assert_eq!(STEAM_APP_ID, 480);
    }
}
