//! WP16 — Steam development identity and release-safety tooling.
//!
//! The App ID source is compiled from the application's module instead of
//! copied here. Development markers therefore cannot drift, while every
//! future package or depot entrypoint has one preflight that rejects Spacewar.

use anyhow::{bail, Context, Result};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;

#[path = "../../crates/solar-sim/src/steam_app_id.rs"]
mod app_id_source;

pub use app_id_source::STEAM_APP_ID;

const DEVELOPMENT_MARKER_FILENAME: &str = "steam_appid.txt";
#[cfg(any(target_os = "macos", test))]
const MACOS_STEAM_ENTITLEMENTS: &str = "../crates/solar-sim/steam-dev.entitlements";

/// Release-sensitive operations that must never target the interim App ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseAction {
    Package,
    Depot,
}

impl ReleaseAction {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "package" => Some(Self::Package),
            "depot" => Some(Self::Depot),
            _ => None,
        }
    }
}

impl fmt::Display for ReleaseAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Package => "package",
            Self::Depot => "depot",
        })
    }
}

/// Writes the development App-ID marker beside an already-built application.
pub fn write_development_app_id(application: &Path) -> Result<PathBuf> {
    if !application.is_file() {
        bail!(
            "Steam development application does not exist: {}",
            application.display()
        );
    }
    let directory = application.parent().unwrap_or_else(|| Path::new("."));
    let output = directory.join(DEVELOPMENT_MARKER_FILENAME);
    fs::write(&output, format!("{STEAM_APP_ID}\n"))
        .with_context(|| format!("write generated Steam App ID to {}", output.display()))?;
    Ok(output)
}

/// Prepares a local executable for Steam client and overlay development.
pub fn prepare_development_application(application: &Path) -> Result<PathBuf> {
    let marker = write_development_app_id(application)?;
    #[cfg(target_os = "macos")]
    sign_macos_development_application(application)?;
    Ok(marker)
}

#[cfg(target_os = "macos")]
fn sign_macos_development_application(application: &Path) -> Result<()> {
    let entitlements = Path::new(env!("CARGO_MANIFEST_DIR")).join(MACOS_STEAM_ENTITLEMENTS);
    let status = Command::new("/usr/bin/codesign")
        .arg("--force")
        .arg("--sign")
        .arg("-")
        .arg("--entitlements")
        .arg(&entitlements)
        .arg(application)
        .status()
        .with_context(|| format!("launch codesign for {}", application.display()))?;
    if !status.success() {
        bail!(
            "codesign failed for Steam development application {} with {}",
            application.display(),
            entitlements.display()
        );
    }
    println!(
        "signed {} for Steam overlay injection with {}",
        application.display(),
        entitlements.display()
    );
    Ok(())
}

/// Guards every packaging and SteamPipe entrypoint against the interim ID.
pub fn require_release_app_id(action: ReleaseAction) -> Result<()> {
    require_release_app_id_value(STEAM_APP_ID, action)
}

fn require_release_app_id_value(app_id: u32, action: ReleaseAction) -> Result<()> {
    if app_id == 480 {
        bail!(
            "Steam {action} refused: App ID 480 is the interim Spacewar development ID; assign the real App ID before packaging or building depots"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

    fn temporary_directory() -> PathBuf {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "solar-sim-steam-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn development_marker_is_generated_beside_the_app_from_the_constant() {
        let directory = temporary_directory();
        let application = directory.join("solar-sim");
        fs::write(&application, b"test executable").unwrap();

        let output = write_development_app_id(&application).unwrap();

        assert_eq!(output, directory.join(DEVELOPMENT_MARKER_FILENAME));
        assert_eq!(
            fs::read_to_string(output).unwrap(),
            format!("{STEAM_APP_ID}\n")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn macos_development_entitlements_allow_steam_overlay_injection() {
        let entitlements = Path::new(env!("CARGO_MANIFEST_DIR")).join(MACOS_STEAM_ENTITLEMENTS);
        let contents = fs::read_to_string(entitlements).unwrap();
        assert!(contents.contains("com.apple.security.cs.disable-library-validation"));
        assert!(contents.contains("com.apple.security.cs.allow-dyld-environment-variables"));
    }

    #[test]
    fn interim_spacewar_id_blocks_both_release_actions() {
        for action in [ReleaseAction::Package, ReleaseAction::Depot] {
            let error = require_release_app_id(action).unwrap_err().to_string();
            assert!(error.contains("App ID 480 is the interim Spacewar development ID"));
        }
    }

    #[test]
    fn real_app_id_will_pass_the_shared_release_preflight() {
        for action in [ReleaseAction::Package, ReleaseAction::Depot] {
            assert!(require_release_app_id_value(481, action).is_ok());
        }
    }
}
