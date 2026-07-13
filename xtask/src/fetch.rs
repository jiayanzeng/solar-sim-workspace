//! Source access. Two implementations behind one trait so the whole pipeline
//! runs bit-identically offline (fixtures) and online (JPL APIs), which is
//! what makes the smoke test and future determinism audits possible.

use anyhow::{Context, Result};
#[cfg(any(feature = "online", test))]
use std::path::Path;
use std::path::PathBuf;

pub trait Fetch {
    /// `cache_key` is the body id; fixtures live at `<dir>/<cache_key>.json`.
    fn get(&self, url: &str, cache_key: &str) -> Result<String>;
    /// True if this source can be probed for availability per key (fixtures).
    fn has(&self, cache_key: &str) -> bool;
    /// Whether this source may perform auxiliary online resolution requests.
    fn is_online(&self) -> bool {
        false
    }
}

/// Offline source: pre-captured API responses. Missing files are reported via
/// `has()` so `--allow-partial` runs can skip bodies cleanly.
pub struct Fixtures {
    pub dir: PathBuf,
}

impl Fetch for Fixtures {
    fn get(&self, _url: &str, cache_key: &str) -> Result<String> {
        let p = self.dir.join(format!("{cache_key}.json"));
        std::fs::read_to_string(&p).with_context(|| format!("fixture missing: {}", p.display()))
    }
    fn has(&self, cache_key: &str) -> bool {
        self.dir.join(format!("{cache_key}.json")).exists()
    }
}

/// Live JPL access. Feature-gated so default builds and CI stay offline.
#[cfg(feature = "online")]
pub struct Http {
    pub capture_dir: Option<PathBuf>,
}

#[cfg(any(feature = "online", test))]
fn write_capture(dir: &Path, cache_key: &str, body: &str) -> Result<()> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("create capture directory {}", dir.display()))?;
    let path = dir.join(format!("{cache_key}.json"));
    std::fs::write(&path, body)
        .with_context(|| format!("write raw response capture {}", path.display()))
}

#[cfg(feature = "online")]
impl Fetch for Http {
    fn get(&self, url: &str, cache_key: &str) -> Result<String> {
        let body = ureq::get(url)
            .timeout(std::time::Duration::from_secs(60))
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_string()?;
        if let Some(dir) = &self.capture_dir {
            write_capture(dir, cache_key, &body)?;
        }
        Ok(body)
    }
    fn has(&self, _cache_key: &str) -> bool {
        true
    }
    fn is_online(&self) -> bool {
        true
    }
}

/// Minimal percent-encoding for JPL query parameter values.
pub fn enc(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '\'' => out.push_str("%27"),
            '+' => out.push_str("%2B"),
            '/' => out.push_str("%2F"),
            '&' => out.push_str("%26"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "solar-sim-capture-test-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn capture_writes_the_exact_raw_response_by_body_id() {
        let dir = temp_dir();
        let body = "{\"result\":\"raw JPL response\\n\"}";
        write_capture(&dir, "jupiter", body).unwrap();

        let path = dir.join("jupiter.json");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), body);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
