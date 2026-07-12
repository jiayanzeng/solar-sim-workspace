//! Normalization and fitting. Everything here is deterministic pure math —
//! the part of the pipeline the §1.4-style governance cares most about.

use anyhow::{anyhow, Result};
use sim_core::catalog::{Elements, Orbit, SecularRates, DAYS_PER_JULIAN_CENTURY};

/// Raw osculating elements as parsed from a single Horizons record (km, deg).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawElements {
    pub a_km: f64,
    pub e: f64,
    pub i_deg: f64,
    pub raan_deg: f64,
    pub argp_deg: f64,
    pub m0_deg: f64,
}

impl RawElements {
    pub fn to_elements(self) -> Elements {
        Elements {
            a_km: self.a_km,
            e: self.e,
            i_deg: self.i_deg,
            raan_deg: self.raan_deg,
            argp_deg: self.argp_deg,
            m0_deg: self.m0_deg,
        }
    }
}

/// Unwrap an angle series (degrees) so consecutive samples never jump by more
/// than 180° — prerequisite for fitting linear rates through 0/360 crossings.
pub fn unwrap_deg_series(vals: &mut [f64]) {
    for i in 1..vals.len() {
        while vals[i] - vals[i - 1] > 180.0 {
            vals[i] -= 360.0;
        }
        while vals[i] - vals[i - 1] < -180.0 {
            vals[i] += 360.0;
        }
    }
}

/// Least-squares line fit. Returns (slope, intercept); None if degenerate.
pub fn fit_linear(xs: &[f64], ys: &[f64]) -> Option<(f64, f64)> {
    if xs.len() < 2 || xs.len() != ys.len() {
        return None;
    }
    let n = xs.len() as f64;
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let (mut sxx, mut sxy) = (0.0, 0.0);
    for (&x, &y) in xs.iter().zip(ys) {
        sxx += (x - mx) * (x - mx);
        sxy += (x - mx) * (y - my);
    }
    if sxx <= 0.0 {
        return None;
    }
    let slope = sxy / sxx;
    Some((slope, my - slope * mx))
}

/// Minimum time span (days) before we trust a secular fit — anything shorter
/// is dominated by short-period perturbations, not secular drift.
const SECULAR_MIN_SPAN_DAYS: f64 = 50.0 * 365.25;
/// Records within this many days of the target epoch feed the mean-motion fit.
const MEAN_MOTION_WINDOW_DAYS: f64 = 5.0;

pub struct PlanetFit {
    pub orbit: Orbit,
}

/// Fit a planet from Horizons samples (spec §4):
/// - base elements: the record nearest the target epoch (must be < 0.5 d away);
/// - mean motion: slope of unwrapped MA over the near-epoch records (captures
///   the perturbation-averaged rate the way Standish's L-dot does);
/// - secular rates: linear fit of a/e/i/Ω/ω across the coarse 1800–2300
///   samples, `None` when the span is too short (fixtures, smoke tests).
pub fn fit_planet(records: &[(f64, RawElements)], target_epoch_jd: f64) -> Result<PlanetFit> {
    if records.is_empty() {
        return Err(anyhow!("no Horizons records"));
    }
    let base = records
        .iter()
        .min_by(|a, b| {
            (a.0 - target_epoch_jd)
                .abs()
                .partial_cmp(&(b.0 - target_epoch_jd).abs())
                .unwrap()
        })
        .unwrap();
    if (base.0 - target_epoch_jd).abs() > 0.5 {
        return Err(anyhow!(
            "no Horizons record within 0.5 d of target epoch JD {target_epoch_jd}"
        ));
    }

    // Mean motion from near-epoch samples.
    let mut near: Vec<&(f64, RawElements)> = records
        .iter()
        .filter(|(jd, _)| (jd - target_epoch_jd).abs() <= MEAN_MOTION_WINDOW_DAYS)
        .collect();
    near.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mean_motion = if near.len() >= 2 {
        let xs: Vec<f64> = near.iter().map(|(jd, _)| *jd).collect();
        let mut ma: Vec<f64> = near.iter().map(|(_, r)| r.m0_deg).collect();
        unwrap_deg_series(&mut ma);
        fit_linear(&xs, &ma).map(|(slope, _)| slope).filter(|s| *s > 0.0)
    } else {
        None
    };

    // Secular rates from the coarse span.
    let mut coarse: Vec<&(f64, RawElements)> = records.iter().collect();
    coarse.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let span = coarse.last().unwrap().0 - coarse.first().unwrap().0;
    let secular = if span >= SECULAR_MIN_SPAN_DAYS && coarse.len() >= 3 {
        let cys: Vec<f64> = coarse
            .iter()
            .map(|(jd, _)| (jd - base.0) / DAYS_PER_JULIAN_CENTURY)
            .collect();
        let fit_field = |get: fn(&RawElements) -> f64, wrap: bool| -> f64 {
            let mut ys: Vec<f64> = coarse.iter().map(|(_, r)| get(r)).collect();
            if wrap {
                unwrap_deg_series(&mut ys);
            }
            fit_linear(&cys, &ys).map(|(s, _)| s).unwrap_or(0.0)
        };
        Some(SecularRates {
            a_km_per_cy: fit_field(|r| r.a_km, false),
            e_per_cy: fit_field(|r| r.e, false),
            i_deg_per_cy: fit_field(|r| r.i_deg, false),
            raan_deg_per_cy: fit_field(|r| r.raan_deg, true),
            argp_deg_per_cy: fit_field(|r| r.argp_deg, true),
        })
    } else {
        None
    };

    Ok(PlanetFit {
        orbit: Orbit {
            epoch_jd_tdb: base.0,
            elements: base.1.to_elements(),
            secular,
            mean_motion_deg_per_day: mean_motion,
        },
    })
}

/// A single-record moon orbit (no secular, no fitted mean motion — the
/// runtime derives n from the parent GM).
pub fn moon_orbit(records: &[(f64, RawElements)], target_epoch_jd: f64) -> Result<Orbit> {
    let base = records
        .iter()
        .min_by(|a, b| {
            (a.0 - target_epoch_jd)
                .abs()
                .partial_cmp(&(b.0 - target_epoch_jd).abs())
                .unwrap()
        })
        .ok_or_else(|| anyhow!("no Horizons records"))?;
    if (base.0 - target_epoch_jd).abs() > 0.5 {
        return Err(anyhow!("no record within 0.5 d of target epoch"));
    }
    Ok(Orbit {
        epoch_jd_tdb: base.0,
        elements: base.1.to_elements(),
        secular: None,
        mean_motion_deg_per_day: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwrap_handles_wraparound() {
        let mut v = vec![350.0, 5.0, 20.0, 355.0];
        unwrap_deg_series(&mut v);
        assert_eq!(v, vec![350.0, 365.0, 380.0, 355.0]);
    }

    #[test]
    fn linear_fit_recovers_slope() {
        let xs = [0.0, 1.0, 2.0, 3.0];
        let ys = [1.0, 3.0, 5.0, 7.0];
        let (s, b) = fit_linear(&xs, &ys).unwrap();
        assert!((s - 2.0).abs() < 1e-12);
        assert!((b - 1.0).abs() < 1e-12);
    }

    fn raw(ma: f64) -> RawElements {
        RawElements { a_km: 1.5e8, e: 0.016, i_deg: 0.0, raan_deg: 100.0, argp_deg: 200.0, m0_deg: ma }
    }

    #[test]
    fn planet_fit_extracts_mean_motion_from_near_records() {
        let recs = vec![(2461042.0, raw(357.5)), (2461043.0, raw(358.4856))];
        let fit = fit_planet(&recs, 2461042.0).unwrap();
        let n = fit.orbit.mean_motion_deg_per_day.unwrap();
        assert!((n - 0.9856).abs() < 1e-9);
        assert!(fit.orbit.secular.is_none(), "1-day span must not yield secular rates");
        assert!((fit.orbit.epoch_jd_tdb - 2461042.0).abs() < 1e-9);
    }

    #[test]
    fn planet_fit_mean_motion_survives_360_crossing() {
        let recs = vec![(2461042.0, raw(359.6)), (2461043.0, raw(0.5856))];
        let fit = fit_planet(&recs, 2461042.0).unwrap();
        let n = fit.orbit.mean_motion_deg_per_day.unwrap();
        assert!((n - 0.9856).abs() < 1e-9);
    }

    #[test]
    fn planet_fit_secular_from_coarse_span() {
        // synthetic: raan drifts −0.24 deg/cy across 1800–2300, plus epoch pair
        let mut recs = Vec::new();
        for k in 0..11 {
            let jd = 2451545.0 + (1800.0 + 50.0 * k as f64 - 2000.0) * 365.25;
            let cy = (jd - 2461042.0) / DAYS_PER_JULIAN_CENTURY;
            recs.push((jd, RawElements { raan_deg: 100.0 - 0.24 * cy, ..raw(0.0) }));
        }
        recs.push((2461042.0, raw(10.0)));
        recs.push((2461043.0, raw(10.9856)));
        let fit = fit_planet(&recs, 2461042.0).unwrap();
        let sec = fit.orbit.secular.unwrap();
        assert!((sec.raan_deg_per_cy + 0.24).abs() < 1e-6, "got {}", sec.raan_deg_per_cy);
        assert!((sec.a_km_per_cy).abs() < 1e-6);
    }

    #[test]
    fn planet_fit_requires_record_at_epoch() {
        let recs = vec![(2461050.0, raw(0.0)), (2461051.0, raw(1.0))];
        assert!(fit_planet(&recs, 2461042.0).is_err());
    }
}
