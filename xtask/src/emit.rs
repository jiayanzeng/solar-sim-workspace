//! Catalog emission: validated `Catalog` → committed `assets/catalog.ron`.
//! Every emitted file re-runs the full sim-core validation first — the
//! generator can never write a file the app would refuse to load.

use anyhow::{anyhow, Result};
use sim_core::catalog::Catalog;
use std::path::Path;

pub fn write_catalog(catalog: &Catalog, path: &Path, invocation: &str) -> Result<()> {
    if let Err(errs) = catalog.validate() {
        let joined = errs.iter().map(|e| format!("  - {e}")).collect::<Vec<_>>().join("\n");
        return Err(anyhow!("refusing to emit invalid catalog:\n{joined}"));
    }
    let body = catalog
        .to_ron_string()
        .map_err(|e| anyhow!("RON serialization failed: {e}"))?;
    let header = format!(
        "// GENERATED FILE — do not edit by hand (Rev B §4.2).\n\
         // Regenerate with: {invocation}\n\
         // Frame: {frame}. Units: km, degrees, Julian Date (TDB).\n\
         // Per-body provenance is in each record's `source` field.\n\
         // Generated: {ts}\n",
        invocation = invocation,
        frame = catalog.frame,
        ts = catalog.generated_utc,
    );
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, format!("{header}{body}\n"))?;
    Ok(())
}

/// ISO-8601 UTC timestamp from the system clock (no chrono dependency;
/// Howard Hinnant's civil-from-days algorithm).
pub fn now_utc_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let (y, m, d) = civil_from_days(days);
    let rem = secs % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_epoch_days() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1)); // 2024-01-01
    }

    #[test]
    fn timestamp_shape() {
        let t = now_utc_iso8601();
        assert_eq!(t.len(), 20);
        assert!(t.ends_with('Z'));
    }
}
