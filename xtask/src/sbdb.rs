//! JPL Small-Body Database access (`sbdb.api`) — dwarf planets, asteroids,
//! comets. SBDB orbits are heliocentric ecliptic-J2000 osculating elements,
//! epochs in JD TDB, `a`/`q` in AU, angles in degrees: one AU→km scale is the
//! only unit conversion (spec §5 lists the time-scale caveats).

use anyhow::{anyhow, Context, Result};
use sim_core::catalog::{Elements, Orbit, AU_KM};

pub const SBDB_API: &str = "https://ssd-api.jpl.nasa.gov/sbdb.api";

pub fn sbdb_url(sstr: &str) -> String {
    format!("{SBDB_API}?sstr={}&full-prec=true", crate::fetch::enc(sstr))
}

#[derive(Debug, Clone, Default)]
pub struct SbdbOrbit {
    pub epoch_jd_tdb: f64,
    pub e: f64,
    pub i_deg: f64,
    pub raan_deg: f64,
    pub argp_deg: f64,
    pub a_au: Option<f64>,
    pub q_au: Option<f64>,
    pub ma_deg: Option<f64>,
    pub tp_jd_tdb: Option<f64>,
}

pub fn parse_response(json_body: &str) -> Result<SbdbOrbit> {
    let v: serde_json::Value =
        serde_json::from_str(json_body).context("SBDB response is not JSON")?;
    let orbit = v
        .get("orbit")
        .ok_or_else(|| anyhow!("SBDB JSON missing 'orbit'"))?;

    let epoch: f64 = num(orbit
        .get("epoch")
        .ok_or_else(|| anyhow!("SBDB orbit missing 'epoch'"))?)?;

    let elements = orbit
        .get("elements")
        .and_then(|e| e.as_array())
        .ok_or_else(|| anyhow!("SBDB orbit missing 'elements' array"))?;

    let mut out = SbdbOrbit {
        epoch_jd_tdb: epoch,
        ..Default::default()
    };
    let mut have = std::collections::HashSet::new();
    for el in elements {
        let name = el.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if !matches!(name, "e" | "i" | "om" | "w" | "a" | "q" | "ma" | "tp") {
            continue;
        }
        let val = match el.get("value") {
            Some(v) if !v.is_null() => num(v)?,
            _ => continue,
        };
        have.insert(name.to_string());
        match name {
            "e" => out.e = val,
            "i" => out.i_deg = val,
            "om" => out.raan_deg = val,
            "w" => out.argp_deg = val,
            "a" => out.a_au = Some(val),
            "q" => out.q_au = Some(val),
            "ma" => out.ma_deg = Some(val),
            "tp" => out.tp_jd_tdb = Some(val),
            _ => {}
        }
    }
    for req in ["e", "i", "om", "w"] {
        if !have.contains(req) {
            return Err(anyhow!("SBDB elements missing '{req}'"));
        }
    }
    Ok(out)
}

/// Normalize an SBDB orbit into the catalog `Orbit`.
///
/// Rules (spec §5):
/// - `a_km` from `a` if present, else `q/(1−e)` — automatically negative for
///   hyperbolic orbits (e > 1), matching the schema convention.
/// - Mean anomaly: use `ma` at the SBDB epoch when present; otherwise re-base
///   the epoch to perihelion (`epoch := tp`, `m0 := 0`) — exact for comets and
///   numerically cleanest for high-e and hyperbolic orbits.
pub fn to_orbit(o: &SbdbOrbit) -> Result<Orbit> {
    if (o.e - 1.0).abs() < 1e-9 {
        return Err(anyhow!("parabolic orbit (e≈1) unsupported by schema v1"));
    }
    let a_km = match (o.a_au, o.q_au) {
        (Some(a), _) => a * AU_KM,
        (None, Some(q)) => q * AU_KM / (1.0 - o.e),
        (None, None) => return Err(anyhow!("SBDB orbit has neither 'a' nor 'q'")),
    };
    let (epoch, m0) = match o.ma_deg {
        Some(ma) => (o.epoch_jd_tdb, ma),
        None => {
            let tp = o
                .tp_jd_tdb
                .ok_or_else(|| anyhow!("SBDB orbit has neither 'ma' nor 'tp'"))?;
            (tp, 0.0)
        }
    };
    Ok(Orbit {
        epoch_jd_tdb: epoch,
        elements: Elements {
            a_km,
            e: o.e,
            i_deg: o.i_deg,
            raan_deg: o.raan_deg,
            argp_deg: o.argp_deg,
            m0_deg: m0,
        },
        secular: None,
        mean_motion_deg_per_day: None,
    })
}

fn num(v: &serde_json::Value) -> Result<f64> {
    if let Some(f) = v.as_f64() {
        return Ok(f);
    }
    if let Some(s) = v.as_str() {
        return s
            .trim()
            .parse::<f64>()
            .with_context(|| format!("bad number '{s}'"));
    }
    Err(anyhow!("expected number, got {v}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASTEROID: &str = r#"{
        "object": {"fullname": "1 Ceres"},
        "orbit": {"epoch": "2461000.5", "elements": [
            {"name": "e", "value": "0.0785"},
            {"name": "a", "value": "2.767"},
            {"name": "i", "value": "10.59"},
            {"name": "om", "value": "80.30"},
            {"name": "w", "value": "73.60"},
            {"name": "ma", "value": "123.45"}
        ]}}"#;

    const HYPERBOLIC: &str = r#"{
        "object": {"fullname": "3I/ATLAS"},
        "orbit": {"epoch": "2460900.5", "elements": [
            {"name": "e", "value": "6.1"},
            {"name": "q", "value": "1.36"},
            {"name": "i", "value": "175.1"},
            {"name": "om", "value": "322.0"},
            {"name": "w", "value": "128.0"},
            {"name": "tp", "value": "2460978.0"}
        ]}}"#;

    #[test]
    fn asteroid_uses_ma_at_epoch() {
        let o = to_orbit(&parse_response(ASTEROID).unwrap()).unwrap();
        assert!((o.epoch_jd_tdb - 2461000.5).abs() < 1e-9);
        assert!((o.elements.m0_deg - 123.45).abs() < 1e-9);
        assert!((o.elements.a_km - 2.767 * AU_KM).abs() < 1.0);
        assert!(!o.elements.is_hyperbolic());
    }

    #[test]
    fn hyperbolic_rebases_to_perihelion_with_negative_a() {
        let o = to_orbit(&parse_response(HYPERBOLIC).unwrap()).unwrap();
        assert!((o.epoch_jd_tdb - 2460978.0).abs() < 1e-9);
        assert_eq!(o.elements.m0_deg, 0.0);
        assert!(o.elements.a_km < 0.0, "a must be negative for e>1");
        assert!(o.elements.is_hyperbolic());
        // q = a(1−e) must recover the input perihelion distance
        let q = o.elements.periapsis_km() / AU_KM;
        assert!((q - 1.36).abs() < 1e-9);
    }

    #[test]
    fn missing_required_element_is_an_error() {
        let bad = r#"{"orbit":{"epoch":"2461000.5","elements":[{"name":"e","value":"0.1"}]}}"#;
        assert!(parse_response(bad).is_err());
    }

    #[test]
    fn null_unrelated_elements_do_not_reject_a_valid_orbit() {
        let response = r#"{
            "orbit": {"epoch": "2461090.5", "elements": [
                {"name": "e", "value": "6.14"},
                {"name": "a", "value": "-0.263"},
                {"name": "q", "value": "1.356"},
                {"name": "i", "value": "175.1"},
                {"name": "om", "value": "322.1"},
                {"name": "w", "value": "128.0"},
                {"name": "ma", "value": "818.2"},
                {"name": "tp", "value": "2460977.9"},
                {"name": "per", "value": null},
                {"name": "ad", "value": null}
            ]}
        }"#;

        let parsed = parse_response(response).unwrap();
        assert_eq!(parsed.a_au, Some(-0.263));
        assert_eq!(parsed.ma_deg, Some(818.2));
    }
}
