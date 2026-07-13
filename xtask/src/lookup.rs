//! Horizons Lookup API resolution for TNO satellites (WP3 — Rev C §5.3).
//!
//! The lookup is performed against the parent system because JPL classifies
//! these moons as asteroidal-system satellites rather than `group=sat`.
//! Returned SPK IDs are request inputs only; they are never curated data.

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

pub const HORIZONS_LOOKUP_API: &str = "https://ssd.jpl.nasa.gov/api/horizons_lookup.api";
const LOOKUP_API_VERSION: &str = "1.1";

#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedMoon {
    pub command: String,
    pub center: String,
}

#[derive(Deserialize)]
struct LookupResponse {
    signature: LookupSignature,
    #[serde(default)]
    result: Vec<LookupMatch>,
}

#[derive(Deserialize)]
struct LookupSignature {
    version: String,
}

#[derive(Deserialize)]
struct LookupMatch {
    name: String,
    spkid: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    alias: Vec<String>,
}

pub fn lookup_url(parent_sstr: &str) -> String {
    format!(
        "{HORIZONS_LOOKUP_API}?sstr={}&group=mb",
        crate::fetch::enc(parent_sstr)
    )
}

/// Resolve a satellite COMMAND and its parent-primary CENTER from one parent
/// system lookup. Both matches must be unique; ambiguity is a hard error.
pub fn resolve_tno_moon(
    json_body: &str,
    moon_sstr: &str,
    parent_sstr: &str,
) -> Result<ResolvedMoon> {
    let response: LookupResponse =
        serde_json::from_str(json_body).context("Horizons lookup response is not JSON")?;
    if response.signature.version != LOOKUP_API_VERSION {
        bail!(
            "Horizons lookup API version mismatch: expected {LOOKUP_API_VERSION}, found {}",
            response.signature.version
        );
    }

    let primary: Vec<&LookupMatch> = response
        .result
        .iter()
        .filter(|m| m.kind == "asteroidal system primary")
        .collect();
    if primary.len() != 1 {
        bail!(
            "lookup for {parent_sstr} returned {} primary-body matches; expected exactly one",
            primary.len()
        );
    }

    let moon_key = search_key(moon_sstr);
    let satellites: Vec<&LookupMatch> = response
        .result
        .iter()
        .filter(|m| m.kind == "asteroidal system satellite")
        .filter(|m| {
            search_key(&m.name) == moon_key || m.alias.iter().any(|a| search_key(a) == moon_key)
        })
        .collect();
    if satellites.len() != 1 {
        bail!(
            "lookup for {moon_sstr} in {parent_sstr} returned {} satellite matches; expected exactly one",
            satellites.len()
        );
    }

    let primary_id = valid_spkid(&primary[0].spkid, "parent primary")?;
    let satellite_id = valid_spkid(&satellites[0].spkid, "satellite")?;
    Ok(ResolvedMoon {
        command: satellite_id.to_string(),
        center: format!("500@{primary_id}"),
    })
}

fn search_key(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn valid_spkid<'a>(value: &'a str, role: &str) -> Result<&'a str> {
    if value.is_empty() || !value.chars().all(|c| c.is_ascii_digit()) {
        return Err(anyhow!("lookup returned invalid {role} SPK ID '{value}'"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ERIS: &str = r#"{
      "signature":{"version":"1.1"},
      "result":[
        {"name":"Eris (system barycenter)","spkid":"20136199","type":"asteroid barycenter","alias":[]},
        {"name":"Eris (primary body)","spkid":"920136199","type":"asteroidal system primary","alias":[]},
        {"name":"Dysnomia","spkid":"120136199","type":"asteroidal system satellite","alias":["Eris I"]}
      ]
    }"#;

    const HAUMEA: &str = r#"{
      "signature":{"version":"1.1"},
      "result":[
        {"name":"Haumea (primary body)","spkid":"920136108","type":"asteroidal system primary","alias":[]},
        {"name":"Hi'iaka","spkid":"120136108","type":"asteroidal system satellite","alias":["Haumea I"]},
        {"name":"Namaka","spkid":"220136108","type":"asteroidal system satellite","alias":["Haumea II"]}
      ]
    }"#;

    #[test]
    fn resolves_unique_moon_and_parent_primary_ids() {
        assert_eq!(
            resolve_tno_moon(ERIS, "Dysnomia", "Eris").unwrap(),
            ResolvedMoon {
                command: "120136199".into(),
                center: "500@920136199".into()
            }
        );
    }

    #[test]
    fn punctuation_insensitive_name_selects_hiiaka_not_namaka() {
        assert_eq!(
            resolve_tno_moon(HAUMEA, "Hiiaka", "Haumea")
                .unwrap()
                .command,
            "120136108"
        );
        assert_eq!(
            resolve_tno_moon(HAUMEA, "Namaka", "Haumea")
                .unwrap()
                .command,
            "220136108"
        );
    }

    #[test]
    fn rejects_api_version_drift() {
        let drifted = ERIS.replace("\"1.1\"", "\"2.0\"");
        assert!(resolve_tno_moon(&drifted, "Dysnomia", "Eris").is_err());
    }

    #[test]
    fn rejects_ambiguous_or_incomplete_lookup_results() {
        let ambiguous = r#"{
          "signature":{"version":"1.1"},
          "result":[
            {"name":"Eris (primary body)","spkid":"920136199","type":"asteroidal system primary","alias":[]},
            {"name":"Dysnomia","spkid":"120136199","type":"asteroidal system satellite","alias":[]},
            {"name":"Dysnomia","spkid":"220136199","type":"asteroidal system satellite","alias":[]}
          ]
        }"#;
        assert!(resolve_tno_moon(ambiguous, "Dysnomia", "Eris").is_err());

        let no_primary = ERIS.replace("asteroidal system primary", "asteroid primary");
        assert!(resolve_tno_moon(&no_primary, "Dysnomia", "Eris").is_err());
    }
}
