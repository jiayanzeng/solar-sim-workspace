//! WP2 — Kepler propagation (Rev B §5, §7).
//!
//! Closed-form two-body evaluation: catalog `Elements` + parent GM + absolute
//! sim time → position/velocity in the parent's ecliptic-J2000 frame, f64,
//! km and km/s. O(1) in time — evaluating year 2299 costs what 2026 costs —
//! which is the entire architectural justification for the ±100 yr/s ladder.
//!
//! Both conic branches are supported: elliptic (0 ≤ e < 1, a > 0) and
//! hyperbolic (e > 1, a < 0, for 3I/ATLAS). Parabolic (e ≈ 1) is rejected,
//! matching the schema. Solvers are Newton with a guaranteed bracketed-
//! bisection fallback — both Kepler equations are strictly monotone in their
//! anomaly, so a bracket always converges and `NoConvergence` is a true
//! "should never happen" guard, not a shrug.

use crate::catalog::{Elements, Orbit, SecularRates, DAYS_PER_JULIAN_CENTURY, SECONDS_PER_DAY};
use crate::time::t_from_jd_tdb;
use std::f64::consts::{PI, TAU};

// ---------------------------------------------------------------------------
// Errors and guards
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeplerError {
    /// NaN/inf in elements, μ, or time.
    NonFinite,
    /// μ must be positive.
    BadMu,
    /// e < 0, or e within `PARABOLIC_WINDOW` of 1 (schema v1 has no consumer).
    UnsupportedEccentricity,
    /// sign(a) disagrees with the branch implied by e.
    AxisEccentricityMismatch,
    /// Solver failed to meet tolerance (unreachable given the bisection
    /// fallback on a valid bracket; kept as a hard guard).
    NoConvergence,
}

impl std::fmt::Display for KeplerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            KeplerError::NonFinite => "non-finite input",
            KeplerError::BadMu => "gravitational parameter must be positive",
            KeplerError::UnsupportedEccentricity => "eccentricity unsupported (e < 0 or e ≈ 1)",
            KeplerError::AxisEccentricityMismatch => "sign(a) inconsistent with e",
            KeplerError::NoConvergence => "anomaly solver failed to converge",
        };
        f.write_str(s)
    }
}

const PARABOLIC_WINDOW: f64 = 1.0e-9;
const NEWTON_MAX_ITERS: usize = 60;
const BISECT_MAX_ITERS: usize = 200;

fn check_elements(el: &Elements, mu: f64) -> Result<(), KeplerError> {
    let finite = [el.a_km, el.e, el.i_deg, el.raan_deg, el.argp_deg, el.m0_deg, mu]
        .iter()
        .all(|v| v.is_finite());
    if !finite {
        return Err(KeplerError::NonFinite);
    }
    if mu <= 0.0 {
        return Err(KeplerError::BadMu);
    }
    if el.e < 0.0 || (el.e - 1.0).abs() < PARABOLIC_WINDOW {
        return Err(KeplerError::UnsupportedEccentricity);
    }
    if (el.e < 1.0 && el.a_km <= 0.0) || (el.e > 1.0 && el.a_km >= 0.0) {
        return Err(KeplerError::AxisEccentricityMismatch);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Anomaly solvers
// ---------------------------------------------------------------------------

/// Solve M = E − e·sin E for E, with M in radians (any magnitude), 0 ≤ e < 1.
/// Returns E consistent with M wrapped into (−π, π] plus the removed turns,
/// so `E − M` is always small — callers get a numerically tame anomaly.
pub fn solve_elliptic(m_rad: f64, e: f64) -> Result<f64, KeplerError> {
    if !m_rad.is_finite() || !(0.0..1.0).contains(&e) {
        return Err(if e >= 1.0 || e < 0.0 {
            KeplerError::UnsupportedEccentricity
        } else {
            KeplerError::NonFinite
        });
    }
    // Wrap to (−π, π]; remember whole turns so E is continuous in M.
    let turns = (m_rad / TAU).round();
    let m = m_rad - turns * TAU;

    if e == 0.0 {
        return Ok(m + turns * TAU);
    }

    let f = |x: f64| x - e * x.sin() - m;

    // Newton from the classic starter (E = M for moderate e; ±π for high e
    // where the M-starter can stall near M ≈ 0 ± e).
    let mut x = if e < 0.8 { m } else { PI.copysign(if m == 0.0 { 1.0 } else { m }) };
    let tol = 1.0e-14 * (1.0 + m.abs());
    for _ in 0..NEWTON_MAX_ITERS {
        let fx = f(x);
        if fx.abs() <= tol {
            return Ok(x + turns * TAU);
        }
        let d = 1.0 - e * x.cos(); // ≥ 1 − e > 0: never divides by zero
        let step = fx / d;
        x -= step;
        if step.abs() <= 1.0e-15 * (1.0 + x.abs()) {
            return Ok(x + turns * TAU);
        }
    }

    // Guaranteed fallback: |E − M| = e|sin E| ≤ e brackets the root, and
    // f is strictly increasing (f' = 1 − e·cos E ≥ 1 − e > 0).
    let (mut lo, mut hi) = (m - e, m + e);
    for _ in 0..BISECT_MAX_ITERS {
        let mid = 0.5 * (lo + hi);
        let fm = f(mid);
        if fm.abs() <= tol || (hi - lo) < 1.0e-15 {
            return Ok(mid + turns * TAU);
        }
        if fm < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Err(KeplerError::NoConvergence)
}

/// Solve M = e·sinh H − H for H, with e > 1. M is unbounded (no wrapping —
/// hyperbolic flybys don't repeat).
pub fn solve_hyperbolic(m_rad: f64, e: f64) -> Result<f64, KeplerError> {
    if !m_rad.is_finite() {
        return Err(KeplerError::NonFinite);
    }
    if e <= 1.0 {
        return Err(KeplerError::UnsupportedEccentricity);
    }
    if m_rad == 0.0 {
        return Ok(0.0);
    }
    // Solve for |M| and mirror: the equation is odd in (M, H).
    let m = m_rad.abs();
    let f = |x: f64| e * x.sinh() - x - m;

    let mut x = (m / e).asinh(); // log-accurate for large M, ~M/e for small
    let tol = 1.0e-12 * (1.0 + m);
    let mut converged = None;
    for _ in 0..NEWTON_MAX_ITERS {
        let fx = f(x);
        if fx.abs() <= tol {
            converged = Some(x);
            break;
        }
        let d = e * x.cosh() - 1.0; // ≥ e − 1 > 0
        let step = fx / d;
        x -= step;
        if step.abs() <= 1.0e-15 * (1.0 + x.abs()) {
            converged = Some(x);
            break;
        }
    }
    if converged.is_none() {
        // Bracket: sinh H ≥ H gives f(M/(e−1)) ≥ 0; sinh H ≥ (M/e at the
        // asinh point) gives f(asinh(M/e)) ≤ 0. f strictly increasing.
        let (mut lo, mut hi) = ((m / e).asinh(), m / (e - 1.0));
        for _ in 0..BISECT_MAX_ITERS {
            let mid = 0.5 * (lo + hi);
            let fm = f(mid);
            if fm.abs() <= tol || (hi - lo) < 1.0e-15 * (1.0 + hi.abs()) {
                converged = Some(mid);
                break;
            }
            if fm < 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
    }
    match converged {
        Some(h) => Ok(h.copysign(m_rad)),
        None => Err(KeplerError::NoConvergence),
    }
}

// ---------------------------------------------------------------------------
// State evaluation
// ---------------------------------------------------------------------------

/// Position (km) and velocity (km/s) in the parent's ecliptic-J2000 frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StateVector {
    pub position_km: [f64; 3],
    pub velocity_km_s: [f64; 3],
}

impl StateVector {
    pub fn radius_km(&self) -> f64 {
        norm(self.position_km)
    }
    pub fn speed_km_s(&self) -> f64 {
        norm(self.velocity_km_s)
    }
}

// Small f64 vector helpers — WP4's propagation uses these too.
pub fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// Perifocal (PQW) basis vectors in the ecliptic frame from Ω, i, ω —
/// the standard 3-1-3 rotation. Retrograde orbits (i > 90°) fall out of the
/// same formulas with no special-casing; the tests pin that down.
fn pqw_basis(el: &Elements) -> ([f64; 3], [f64; 3]) {
    let (so, co) = el.raan_deg.to_radians().sin_cos();
    let (si, ci) = el.i_deg.to_radians().sin_cos();
    let (sw, cw) = el.argp_deg.to_radians().sin_cos();
    let p = [co * cw - so * sw * ci, so * cw + co * sw * ci, sw * si];
    let q = [-co * sw - so * cw * ci, -so * sw + co * cw * ci, cw * si];
    (p, q)
}

/// Evaluate the state from elements at a given mean anomaly, using mean
/// motion `n_rad_s` (the fitted override when the catalog carries one).
/// Velocities are derived from the same `n` that advances M, so velocity is
/// exactly d(position)/dt even when the override differs from √(μ/|a|³) —
/// the finite-difference test enforces this.
pub fn state_from_elements(
    el: &Elements,
    mu_km3_s2: f64,
    n_rad_s: f64,
    m_rad: f64,
) -> Result<StateVector, KeplerError> {
    check_elements(el, mu_km3_s2)?;
    if !m_rad.is_finite() || !n_rad_s.is_finite() || n_rad_s <= 0.0 {
        return Err(KeplerError::NonFinite);
    }
    let a = el.a_km;
    let e = el.e;
    let na2 = n_rad_s * a * a;

    // Perifocal position (xp, yp) and velocity (vxp, vyp).
    let (xp, yp, vxp, vyp) = if e < 1.0 {
        let big_e = solve_elliptic(m_rad, e)?;
        let (s, c) = big_e.sin_cos();
        let beta = (1.0 - e * e).sqrt();
        let r = a * (1.0 - e * c);
        (a * (c - e), a * beta * s, -na2 * s / r, na2 * beta * c / r)
    } else {
        let h = solve_hyperbolic(m_rad, e)?;
        let (sh, ch) = (h.sinh(), h.cosh());
        let beta = (e * e - 1.0).sqrt();
        let r = a * (1.0 - e * ch); // = |a|(e·cosh H − 1) > 0 since a < 0
        (a * (ch - e), -a * beta * sh, -na2 * sh / r, na2 * beta * ch / r)
    };

    let (p, q) = pqw_basis(el);
    Ok(StateVector {
        position_km: [
            xp * p[0] + yp * q[0],
            xp * p[1] + yp * q[1],
            xp * p[2] + yp * q[2],
        ],
        velocity_km_s: [
            vxp * p[0] + vyp * q[0],
            vxp * p[1] + vyp * q[1],
            vxp * p[2] + vyp * q[2],
        ],
    })
}

/// Elements drifted to time `t_s` (seconds since J2000, TDB) by the linear
/// secular rates, when present. `m0_deg` is untouched — mean-anomaly
/// propagation is the caller's job via mean motion.
pub fn elements_at(orbit: &Orbit, t_s: f64) -> Elements {
    let mut el = orbit.elements;
    if let Some(SecularRates {
        a_km_per_cy,
        e_per_cy,
        i_deg_per_cy,
        raan_deg_per_cy,
        argp_deg_per_cy,
    }) = orbit.secular
    {
        let cy = (t_s - t_from_jd_tdb(orbit.epoch_jd_tdb))
            / (DAYS_PER_JULIAN_CENTURY * SECONDS_PER_DAY);
        el.a_km += a_km_per_cy * cy;
        el.e = (el.e + e_per_cy * cy).max(0.0);
        el.i_deg += i_deg_per_cy * cy;
        el.raan_deg += raan_deg_per_cy * cy;
        el.argp_deg += argp_deg_per_cy * cy;
    }
    el
}

/// The one call WP4's propagation loop makes per body per frame: catalog
/// orbit + parent μ + absolute sim time → parent-frame state.
pub fn state_at(orbit: &Orbit, mu_km3_s2: f64, t_s: f64) -> Result<StateVector, KeplerError> {
    if !t_s.is_finite() {
        return Err(KeplerError::NonFinite);
    }
    let el = elements_at(orbit, t_s);
    let n = orbit.mean_motion_rad_per_s(mu_km3_s2);
    let dt = t_s - t_from_jd_tdb(orbit.epoch_jd_tdb);
    let m = el.m0_deg.to_radians() + n * dt;
    state_from_elements(&el, mu_km3_s2, n, m)
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Orbit;

    const MU_SUN: f64 = 1.327_124_400_18e11;
    const MU_NEPTUNE: f64 = 6.836_529e6;

    fn orbit(el: Elements) -> Orbit {
        Orbit {
            epoch_jd_tdb: 2_461_042.0,
            elements: el,
            secular: None,
            mean_motion_deg_per_day: None,
        }
    }

    // ---- convergence sweeps (WP2 acceptance) ----

    #[test]
    fn elliptic_convergence_sweep() {
        let es = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 0.97];
        for &e in &es {
            for k in 0..=720 {
                let m = -TAU + (k as f64) * (2.0 * TAU / 720.0); // [−2π, 2π]
                let big_e = solve_elliptic(m, e).unwrap();
                let resid = big_e - e * big_e.sin() - m;
                assert!(
                    resid.abs() < 1e-12,
                    "e={e} M={m}: residual {resid:e}"
                );
            }
            // huge M (±100 yr/s territory): residual vs the same unwrapped M
            for &m in &[1.0e6_f64, -3.7e7, 6.5e5 + 0.318] {
                let big_e = solve_elliptic(m, e).unwrap();
                let resid = big_e - e * big_e.sin() - m;
                assert!(resid.abs() < 1e-7, "e={e} M={m}: residual {resid:e}");
                assert!((big_e - m).abs() <= e + 1e-9, "E stays within e of M");
            }
        }
    }

    #[test]
    fn hyperbolic_convergence_sweep() {
        let es = [1.2, 1.5, 2.0, 3.0, 4.5, 6.0];
        for &e in &es {
            for exp in -6..=4 {
                for &sign in &[1.0, -1.0] {
                    let m = sign * 10.0_f64.powi(exp) * 1.37; // off round numbers
                    let h = solve_hyperbolic(m, e).unwrap();
                    let resid = e * h.sinh() - h - m;
                    assert!(
                        resid.abs() < 1e-9 * (1.0 + m.abs()),
                        "e={e} M={m}: residual {resid:e}"
                    );
                }
            }
            assert_eq!(solve_hyperbolic(0.0, e).unwrap(), 0.0);
        }
    }

    #[test]
    fn solver_guards() {
        assert_eq!(solve_elliptic(1.0, 1.0), Err(KeplerError::UnsupportedEccentricity));
        assert_eq!(solve_elliptic(1.0, -0.1), Err(KeplerError::UnsupportedEccentricity));
        assert_eq!(solve_elliptic(f64::NAN, 0.5), Err(KeplerError::NonFinite));
        assert_eq!(solve_hyperbolic(1.0, 1.0), Err(KeplerError::UnsupportedEccentricity));
        assert_eq!(solve_hyperbolic(f64::INFINITY, 2.0), Err(KeplerError::NonFinite));
    }

    #[test]
    fn state_guards() {
        let good = Elements {
            a_km: 1.5e8, e: 0.1, i_deg: 1.0, raan_deg: 2.0, argp_deg: 3.0, m0_deg: 4.0,
        };
        let o = orbit(good);
        assert!(state_at(&o, MU_SUN, 0.0).is_ok());
        assert_eq!(state_at(&o, -1.0, 0.0), Err(KeplerError::BadMu));
        assert_eq!(state_at(&o, MU_SUN, f64::NAN), Err(KeplerError::NonFinite));

        let mismatch = Elements { a_km: -1.5e8, ..good };
        assert_eq!(
            state_at(&orbit(mismatch), MU_SUN, 0.0),
            Err(KeplerError::AxisEccentricityMismatch)
        );
        let parabolic = Elements { e: 1.0, a_km: -1.5e8, ..good };
        assert_eq!(
            state_at(&orbit(parabolic), MU_SUN, 0.0),
            Err(KeplerError::UnsupportedEccentricity)
        );
    }

    // ---- geometry and invariants ----

    #[test]
    fn circular_orbit_sanity() {
        let el = Elements {
            a_km: 1.0e5, e: 0.0, i_deg: 0.0, raan_deg: 0.0, argp_deg: 0.0, m0_deg: 0.0,
        };
        let o = orbit(el);
        let mu = 3.986e5; // Earth-ish
        for k in 0..8 {
            let t = t_from_jd_tdb(o.epoch_jd_tdb) + k as f64 * 5.0e3;
            let s = state_at(&o, mu, t).unwrap();
            assert!((s.radius_km() - 1.0e5).abs() < 1e-6);
            assert!((s.speed_km_s() - (mu / 1.0e5).sqrt()).abs() < 1e-12);
        }
    }

    /// Energy and angular momentum must be constant along both branches, and
    /// match their closed-form values (vis-viva; h = √(μ·a·(1−e²))).
    #[test]
    fn invariants_elliptic_high_e_and_hyperbolic() {
        let cases = [
            // Halley-like ellipse
            Elements {
                a_km: 2.667e9, e: 0.967, i_deg: 162.26, raan_deg: 58.42,
                argp_deg: 111.33, m0_deg: 0.0,
            },
            // 3I/ATLAS-like hyperbola
            Elements {
                a_km: -3.99e7, e: 6.1, i_deg: 175.1, raan_deg: 322.0,
                argp_deg: 128.0, m0_deg: 0.0,
            },
        ];
        for el in cases {
            let o = orbit(el);
            let energy_expected = -MU_SUN / (2.0 * el.a_km);
            let h_expected = (MU_SUN * el.a_km * (1.0 - el.e * el.e)).sqrt();
            let t0 = t_from_jd_tdb(o.epoch_jd_tdb);
            for k in -40..=40 {
                let t = t0 + k as f64 * 30.0 * SECONDS_PER_DAY;
                let s = state_at(&o, MU_SUN, t).unwrap();
                let r = s.radius_km();
                let energy = 0.5 * s.speed_km_s().powi(2) - MU_SUN / r;
                assert!(
                    (energy / energy_expected - 1.0).abs() < 1e-10,
                    "energy drift at k={k} for e={}", el.e
                );
                let h = norm(cross(s.position_km, s.velocity_km_s));
                assert!(
                    (h / h_expected - 1.0).abs() < 1e-10,
                    "angular momentum drift at k={k} for e={}", el.e
                );
            }
        }
    }

    #[test]
    fn elliptic_period_closure() {
        let el = Elements {
            a_km: 1.495_979e8, e: 0.0167, i_deg: 0.003, raan_deg: 175.0,
            argp_deg: 288.0, m0_deg: 357.5,
        };
        let o = orbit(el);
        let t0 = t_from_jd_tdb(o.epoch_jd_tdb);
        let period = o.period_s(MU_SUN).unwrap();
        let s0 = state_at(&o, MU_SUN, t0).unwrap();
        let s1 = state_at(&o, MU_SUN, t0 + period).unwrap();
        for i in 0..3 {
            assert!((s0.position_km[i] - s1.position_km[i]).abs() < 1e-3, "km closure");
            assert!((s0.velocity_km_s[i] - s1.velocity_km_s[i]).abs() < 1e-9);
        }
    }

    /// Independent cross-validation (the §1.4 dual-source instinct at unit
    /// scale): integrate the raw two-body ODE with RK4 and require the
    /// closed-form path to agree — through perihelion, on both branches.
    #[test]
    fn rk4_cross_validation_both_branches() {
        fn accel(r: [f64; 3], mu: f64) -> [f64; 3] {
            let d = norm(r);
            let k = -mu / (d * d * d);
            [k * r[0], k * r[1], k * r[2]]
        }
        fn rk4(mut s: StateVector, mu: f64, dt: f64, steps: usize) -> StateVector {
            for _ in 0..steps {
                let (r0, v0) = (s.position_km, s.velocity_km_s);
                let a0 = accel(r0, mu);
                let r1 = add(r0, scale(v0, dt / 2.0));
                let v1 = add(v0, scale(a0, dt / 2.0));
                let a1 = accel(r1, mu);
                let r2 = add(r0, scale(v1, dt / 2.0));
                let v2 = add(v0, scale(a1, dt / 2.0));
                let a2 = accel(r2, mu);
                let r3 = add(r0, scale(v2, dt));
                let v3 = add(v0, scale(a2, dt));
                let a3 = accel(r3, mu);
                s.position_km = add(
                    r0,
                    scale(add(add(v0, scale(add(v1, v2), 2.0)), v3), dt / 6.0),
                );
                s.velocity_km_s = add(
                    v0,
                    scale(add(add(a0, scale(add(a1, a2), 2.0)), a3), dt / 6.0),
                );
            }
            s
        }
        fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
            [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
        }
        fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
            [a[0] * k, a[1] * k, a[2] * k]
        }

        let cases = [
            Elements {
                a_km: 2.667e9, e: 0.967, i_deg: 162.26, raan_deg: 58.42,
                argp_deg: 111.33, m0_deg: -1.0, // just before perihelion
            },
            Elements {
                a_km: -3.99e7, e: 6.1, i_deg: 175.1, raan_deg: 322.0,
                argp_deg: 128.0, m0_deg: -25.0, // inbound leg
            },
        ];
        for el in cases {
            let o = orbit(el);
            let t0 = t_from_jd_tdb(o.epoch_jd_tdb);
            let span = 20.0 * SECONDS_PER_DAY; // crosses perihelion in both cases
            let steps = 40_000;
            let s0 = state_at(&o, MU_SUN, t0).unwrap();
            let integrated = rk4(s0, MU_SUN, span / steps as f64, steps);
            let closed = state_at(&o, MU_SUN, t0 + span).unwrap();
            let dr = norm(add(closed.position_km, scale(integrated.position_km, -1.0)));
            assert!(
                dr / closed.radius_km() < 1e-6,
                "e={}: RK4 vs closed-form diverge by {dr} km", el.e
            );
        }
    }

    /// Velocity must be the exact time-derivative of position — including
    /// when a fitted mean-motion override decouples n from √(μ/|a|³).
    #[test]
    fn velocity_is_position_derivative_even_with_override() {
        let mut o = orbit(Elements {
            a_km: 1.495_979e8, e: 0.0167, i_deg: 3.0, raan_deg: 175.0,
            argp_deg: 288.0, m0_deg: 42.0,
        });
        o.mean_motion_deg_per_day = Some(0.9856); // fitted, ≠ √(μ/a³) exactly
        let t = t_from_jd_tdb(o.epoch_jd_tdb) + 1.0e7;
        let h = 1.0;
        let sm = state_at(&o, MU_SUN, t - h).unwrap();
        let sp = state_at(&o, MU_SUN, t + h).unwrap();
        let s = state_at(&o, MU_SUN, t).unwrap();
        for i in 0..3 {
            let fd = (sp.position_km[i] - sm.position_km[i]) / (2.0 * h);
            assert!(
                (fd - s.velocity_km_s[i]).abs() < 1e-6,
                "axis {i}: fd={fd} v={}", s.velocity_km_s[i]
            );
        }
        // and the override actually drives the period
        let period = o.period_s(MU_SUN).unwrap();
        assert!((period / SECONDS_PER_DAY - 360.0 / 0.9856 * 1.0).abs() < 1.0e-6 * period);
    }

    // ---- WP2 acceptance fixtures ----

    /// Retrograde orbits: Triton (i ≈ 157°) and Phoebe (i ≈ 175°) must orbit
    /// backwards — negative z angular momentum — with no special-casing.
    #[test]
    fn retrograde_fixtures_triton_and_phoebe() {
        let cases = [
            ("triton", Elements {
                a_km: 3.548e5, e: 1.6e-5, i_deg: 157.3, raan_deg: 178.1,
                argp_deg: 0.0, m0_deg: 60.0,
            }, MU_NEPTUNE),
            ("phoebe", Elements {
                a_km: 1.2947e7, e: 0.156, i_deg: 175.2, raan_deg: 241.6,
                argp_deg: 342.5, m0_deg: 120.0,
            }, 3.7931e7), // Saturn
        ];
        for (name, el, mu) in cases {
            let o = orbit(el);
            let s = state_at(&o, mu, t_from_jd_tdb(o.epoch_jd_tdb) + 1.0e5).unwrap();
            let h = cross(s.position_km, s.velocity_km_s);
            assert!(h[2] < 0.0, "{name}: retrograde must give h_z < 0, got {}", h[2]);
        }
        // control: a prograde Io-like orbit has h_z > 0
        let io = orbit(Elements {
            a_km: 4.218e5, e: 0.004, i_deg: 2.2, raan_deg: 43.0, argp_deg: 84.0, m0_deg: 200.0,
        });
        let s = state_at(&io, 1.26687e8, 0.0).unwrap();
        assert!(cross(s.position_km, s.velocity_km_s)[2] > 0.0);
    }

    /// Nereid (e ≈ 0.75): the high-eccentricity ellipse of the moon catalog.
    /// Radial extremes must hit a(1∓e) and the period must be ~360 days.
    #[test]
    fn nereid_fixture_high_eccentricity_ellipse() {
        let el = Elements {
            a_km: 5.5134e6, e: 0.7507, i_deg: 7.09, raan_deg: 326.0,
            argp_deg: 290.3, m0_deg: 0.0, // start at perihelion
        };
        let o = orbit(el);
        let t0 = t_from_jd_tdb(o.epoch_jd_tdb);
        let period = o.period_s(MU_NEPTUNE).unwrap();
        assert!(
            (period / SECONDS_PER_DAY - 360.13).abs() < 1.5,
            "Nereid period ≈ 360 d, got {}", period / SECONDS_PER_DAY
        );

        // perihelion at M = 0, aphelion at M = π
        let s_peri = state_at(&o, MU_NEPTUNE, t0).unwrap();
        assert!((s_peri.radius_km() - el.a_km * (1.0 - el.e)).abs() < 1e-3);
        let s_apo = state_at(&o, MU_NEPTUNE, t0 + period / 2.0).unwrap();
        assert!((s_apo.radius_km() - el.a_km * (1.0 + el.e)).abs() < 1e-3);

        // dense sweep: radius stays within [q, Q] and speed is max at q
        let mut r_min = f64::MAX;
        let mut r_max = 0.0_f64;
        for k in 0..=1000 {
            let s = state_at(&o, MU_NEPTUNE, t0 + period * k as f64 / 1000.0).unwrap();
            r_min = r_min.min(s.radius_km());
            r_max = r_max.max(s.radius_km());
        }
        assert!(r_min >= el.a_km * (1.0 - el.e) - 1e-3);
        assert!(r_max <= el.a_km * (1.0 + el.e) + 1e-3);
        assert!(
            s_peri.speed_km_s() > s_apo.speed_km_s() * 5.0,
            "e=0.75: perihelion speed must dwarf aphelion speed"
        );
    }

    /// Hyperbolic geometry: perihelion at M = 0 equals a(1−e) = q, and the
    /// trajectory is time-symmetric about perihelion.
    #[test]
    fn hyperbolic_perihelion_and_symmetry() {
        let el = Elements {
            a_km: -3.99e7, e: 6.1, i_deg: 175.1, raan_deg: 322.0,
            argp_deg: 128.0, m0_deg: 0.0,
        };
        let o = orbit(el);
        let t0 = t_from_jd_tdb(o.epoch_jd_tdb);
        let s = state_at(&o, MU_SUN, t0).unwrap();
        assert!((s.radius_km() - el.periapsis_km()).abs() < 1e-3);
        for k in 1..=10 {
            let dt = k as f64 * 20.0 * SECONDS_PER_DAY;
            let before = state_at(&o, MU_SUN, t0 - dt).unwrap();
            let after = state_at(&o, MU_SUN, t0 + dt).unwrap();
            assert!((before.radius_km() - after.radius_km()).abs() / after.radius_km() < 1e-12);
            assert!(after.radius_km() > s.radius_km(), "receding after perihelion");
        }
    }

    // ---- secular rates ----

    #[test]
    fn secular_rates_drift_elements_linearly() {
        let mut o = orbit(Elements {
            a_km: 1.495_979e8, e: 0.0167, i_deg: 0.003, raan_deg: 175.0,
            argp_deg: 288.0, m0_deg: 0.0,
        });
        o.secular = Some(SecularRates {
            a_km_per_cy: -10.0,
            e_per_cy: 1.0e-5,
            i_deg_per_cy: -0.01,
            raan_deg_per_cy: -0.24,
            argp_deg_per_cy: 0.32,
        });
        let epoch_t = t_from_jd_tdb(o.epoch_jd_tdb);
        let cy = DAYS_PER_JULIAN_CENTURY * SECONDS_PER_DAY;

        let at_epoch = elements_at(&o, epoch_t);
        assert_eq!(at_epoch, o.elements, "no drift at the epoch itself");

        let plus_two = elements_at(&o, epoch_t + 2.0 * cy);
        assert!((plus_two.raan_deg - (175.0 - 0.48)).abs() < 1e-9);
        assert!((plus_two.a_km - (1.495_979e8 - 20.0)).abs() < 1e-6);

        let minus_one = elements_at(&o, epoch_t - cy);
        assert!((minus_one.argp_deg - (288.0 - 0.32)).abs() < 1e-9);

        // drifted elements feed the propagation
        let s_a = state_at(&o, MU_SUN, epoch_t + 2.0 * cy).unwrap();
        o.secular = None;
        let s_b = state_at(&o, MU_SUN, epoch_t + 2.0 * cy).unwrap();
        let dr = [
            s_a.position_km[0] - s_b.position_km[0],
            s_a.position_km[1] - s_b.position_km[1],
            s_a.position_km[2] - s_b.position_km[2],
        ];
        assert!(norm(dr) > 1.0e4, "secular drift must visibly move the position");
    }

    /// Kepler's third law consistency between the solver's n and the
    /// catalog's period helper.
    #[test]
    fn third_law_consistency_with_catalog_period() {
        let el = Elements {
            a_km: 4.218e5, e: 0.004, i_deg: 0.05, raan_deg: 43.0, argp_deg: 84.0, m0_deg: 0.0,
        };
        let o = orbit(el);
        let mu_jupiter = 1.266_865e8;
        let n = o.mean_motion_rad_per_s(mu_jupiter);
        let period = o.period_s(mu_jupiter).unwrap();
        assert!((n * period - TAU).abs() < 1e-12);
        assert!((period / SECONDS_PER_DAY - 1.769).abs() < 0.01, "Io ≈ 1.77 d");
    }
}
