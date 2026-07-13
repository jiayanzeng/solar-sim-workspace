//! JPL Horizons `MAKE_EPHEM / EPHEM_TYPE=ELEMENTS` access.
//!
//! Request contract (spec §4): `format=json`, `REF_PLANE='ECLIPTIC'`,
//! `REF_SYSTEM='J2000'`, `OUT_UNITS='KM-S'`, `TLIST` in JD **TDB**. Angles
//! come back in degrees, distances in km — exactly the catalog's file units.
//! The JSON envelope carries the classic text table in `result`; we parse the
//! records between `$$SOE` and `$$EOE`.

use crate::normalize::RawElements;
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use std::path::Path;

pub const HORIZONS_API: &str = "https://ssd.jpl.nasa.gov/api/horizons.api";

pub fn elements_url(command: &str, center: &str, tlist_jd: &[f64]) -> String {
    let tlist = tlist_jd
        .iter()
        .map(|j| format!("{j:.6}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{HORIZONS_API}?format=json&COMMAND='{}'&OBJ_DATA='NO'&MAKE_EPHEM='YES'\
         &EPHEM_TYPE='ELEMENTS'&CENTER='{}'&REF_PLANE='ECLIPTIC'&REF_SYSTEM='J2000'\
         &OUT_UNITS='KM-S'&TLIST_TYPE='JD'&TLIST='{}'&CSV_FORMAT='NO'",
        crate::fetch::enc(command),
        crate::fetch::enc(center),
        crate::fetch::enc(&tlist)
    )
}

/// Parse a Horizons JSON response into (jd_tdb, elements) records.
pub fn parse_response(json_body: &str) -> Result<Vec<(f64, RawElements)>> {
    let v: serde_json::Value =
        serde_json::from_str(json_body).context("Horizons response is not JSON")?;
    let text = v
        .get("result")
        .and_then(|r| r.as_str())
        .ok_or_else(|| anyhow!("Horizons JSON missing 'result' field"))?;
    parse_elements_text(text)
}

/// Parse one body's response, preserving the exact raw payload on failure so
/// online-capture diagnostics never lose the server's explanatory message.
pub fn parse_response_for_body(json_body: &str, body_id: &str) -> Result<Vec<(f64, RawElements)>> {
    parse_response_with_debug_dir(json_body, body_id, Path::new("target/xtask-debug"))
}

fn parse_response_with_debug_dir(
    json_body: &str,
    body_id: &str,
    debug_dir: &Path,
) -> Result<Vec<(f64, RawElements)>> {
    match parse_response(json_body) {
        Ok(records) => Ok(records),
        Err(parse_error) => {
            let path = debug_dir.join(format!("{body_id}.response.txt"));
            let dump = std::fs::create_dir_all(debug_dir)
                .with_context(|| format!("create debug directory {}", debug_dir.display()))
                .and_then(|()| {
                    std::fs::write(&path, json_body)
                        .with_context(|| format!("write raw response {}", path.display()))
                });
            match dump {
                Ok(()) => Err(parse_error.context(format!(
                    "raw Horizons response written to {}",
                    path.display()
                ))),
                Err(dump_error) => Err(parse_error.context(format!(
                    "failed to preserve raw Horizons response at {}: {dump_error:#}",
                    path.display()
                ))),
            }
        }
    }
}

/// Parse the `$$SOE … $$EOE` element table.
pub fn parse_elements_text(text: &str) -> Result<Vec<(f64, RawElements)>> {
    let soe = text
        .find("$$SOE")
        .ok_or_else(|| anyhow!("no $$SOE in Horizons result"))?;
    let eoe = text
        .find("$$EOE")
        .ok_or_else(|| anyhow!("no $$EOE in Horizons result"))?;
    let block = &text[soe + 5..eoe];

    // Records begin with "<JD> = A.D. ..."
    let header = Regex::new(r"(?m)^\s*(\d{7}\.\d+)\s*=\s*A\.D\.").unwrap();
    // Element pairs; longer keys listed first so alternation can't split them.
    let pair = Regex::new(
        r"\b(EC|QR|IN|OM|Tp|MA|TA|AD|PR|N|A|W)\s*=\s*([-+]?[0-9]*\.?[0-9]+(?:[eE][-+]?[0-9]+)?)",
    )
    .unwrap();

    let headers: Vec<(usize, f64)> = header
        .captures_iter(block)
        .map(|c| {
            let m = c.get(0).unwrap();
            let jd: f64 = c[1].parse().unwrap();
            (m.start(), jd)
        })
        .collect();
    if headers.is_empty() {
        return Err(anyhow!("no element records between $$SOE/$$EOE"));
    }

    let mut out = Vec::with_capacity(headers.len());
    for (i, &(start, jd)) in headers.iter().enumerate() {
        let end = headers.get(i + 1).map(|&(s, _)| s).unwrap_or(block.len());
        let seg = &block[start..end];
        let mut ec = None;
        let mut a = None;
        let mut inc = None;
        let mut om = None;
        let mut w = None;
        let mut ma = None;
        for c in pair.captures_iter(seg) {
            let val: f64 = c[2]
                .parse()
                .with_context(|| format!("bad number in '{}'", &c[0]))?;
            match &c[1] {
                "EC" => ec = Some(val),
                "A" => a = Some(val),
                "IN" => inc = Some(val),
                "OM" => om = Some(val),
                "W" => w = Some(val),
                "MA" => ma = Some(val),
                _ => {} // QR/Tp/TA/AD/PR/N unused: A + MA are authoritative here
            }
        }
        let rec = RawElements {
            a_km: a.ok_or_else(|| anyhow!("record @JD {jd}: missing A"))?,
            e: ec.ok_or_else(|| anyhow!("record @JD {jd}: missing EC"))?,
            i_deg: inc.ok_or_else(|| anyhow!("record @JD {jd}: missing IN"))?,
            raan_deg: om.ok_or_else(|| anyhow!("record @JD {jd}: missing OM"))?,
            argp_deg: w.ok_or_else(|| anyhow!("record @JD {jd}: missing W"))?,
            m0_deg: ma.ok_or_else(|| anyhow!("record @JD {jd}: missing MA"))?,
        };
        out.push((jd, rec));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "solar-sim-horizons-debug-test-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ))
    }

    const SNIPPET: &str = "\
*******************************************************************************
$$SOE
2461042.000000000 = A.D. 2026-Jan-01 12:00:00.0000 TDB
 EC= 1.670000000000000E-02 QR= 1.471000000000000E+08 IN= 3.000000000000000E-03
 OM= 1.750000000000000E+02 W = 2.880000000000000E+02 Tp=  2461045.000000000
 N = 1.140000000000000E-05 MA= 3.575000000000000E+02 TA= 3.573000000000000E+02
 A = 1.495979000000000E+08 AD= 1.521000000000000E+08 PR= 3.155800000000000E+07
2461043.000000000 = A.D. 2026-Jan-02 12:00:00.0000 TDB
 EC= 1.670000000000000E-02 QR= 1.471000000000000E+08 IN= 3.000000000000000E-03
 OM= 1.750000000000000E+02 W = 2.880000000000000E+02 Tp=  2461045.000000000
 N = 1.140000000000000E-05 MA= 3.584856000000000E+02 TA= 3.583000000000000E+02
 A = 1.495979000000000E+08 AD= 1.521000000000000E+08 PR= 3.155800000000000E+07
$$EOE
*******************************************************************************";

    #[test]
    fn parses_two_records() {
        let recs = parse_elements_text(SNIPPET).unwrap();
        assert_eq!(recs.len(), 2);
        assert!((recs[0].0 - 2461042.0).abs() < 1e-9);
        assert!((recs[0].1.a_km - 1.495979e8).abs() < 1.0);
        assert!((recs[0].1.e - 0.0167).abs() < 1e-12);
        assert!((recs[0].1.argp_deg - 288.0).abs() < 1e-9);
        assert!((recs[1].1.m0_deg - 358.4856).abs() < 1e-9);
    }

    #[test]
    fn rejects_missing_markers() {
        assert!(parse_elements_text("no markers here").is_err());
    }

    #[test]
    fn parse_failure_preserves_raw_response_and_reports_its_path() {
        let dir = temp_dir();
        let raw = r#"{"result":"No ephemeris after 2200"}"#;
        let error = parse_response_with_debug_dir(raw, "jupiter", &dir).unwrap_err();
        let path = dir.join("jupiter.response.txt");

        assert_eq!(std::fs::read_to_string(&path).unwrap(), raw);
        assert!(format!("{error:#}").contains(path.to_str().unwrap()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn url_is_stable() {
        let u = elements_url("399", "500@10", &[2461042.0, 2461043.0]);
        assert!(u.contains("COMMAND=%27399%27") || u.contains("COMMAND='399'"));
        assert!(u.contains("ECLIPTIC"));
        assert!(u.contains("2461042.000000"));
    }
}
