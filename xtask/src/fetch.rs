//! Source access. Two implementations behind one trait so the whole pipeline
//! runs bit-identically offline (fixtures) and online (JPL APIs), which is
//! what makes the smoke test and future determinism audits possible.

use anyhow::{Context, Result};
use std::path::PathBuf;

pub trait Fetch {
    /// `cache_key` is the body id; fixtures live at `<dir>/<cache_key>.json`.
    fn get(&self, url: &str, cache_key: &str) -> Result<String>;
    /// True if this source can be probed for availability per key (fixtures).
    fn has(&self, cache_key: &str) -> bool;
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
pub struct Http;

#[cfg(feature = "online")]
impl Fetch for Http {
    fn get(&self, url: &str, _cache_key: &str) -> Result<String> {
        let body = ureq::get(url)
            .timeout(std::time::Duration::from_secs(60))
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_string()?;
        Ok(body)
    }
    fn has(&self, _cache_key: &str) -> bool {
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
