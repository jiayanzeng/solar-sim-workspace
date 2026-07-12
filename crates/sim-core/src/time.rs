//! WP1 — the simulation clock (Rev B §5).
//!
//! One `f64`: TDB seconds since J2000 (JD 2451545.0). A signed rate from the
//! Eyes ladder, play/pause, LIVE detection, an eased snap-to-LIVE, and the
//! 1800–2300 soft range with the 2050 high-confidence boundary. Engine-
//! agnostic: the Bevy `TimePlugin` (WP0/WP8) drives `tick()` once per frame
//! and forwards `SimCommand`s to these methods; nothing here knows about UI.
//!
//! Time-scale stance (visual grade, documented not hidden): the clock runs in
//! TDB like the catalog. Wall-clock "now" arrives as Unix/UTC and is converted
//! with a constant TT−UTC offset (69.184 s, i.e. 37 leap seconds + 32.184 s),
//! ignoring TDB−TT periodic terms (< 2 ms) and any future leap seconds. At
//! prototype accuracy this error is invisible; revisit if V1's ephemeris
//! truth gates ever move in here.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Fundamental constants
// ---------------------------------------------------------------------------

/// Seconds per Julian day / year (the year used by every "yr/s" ladder step).
pub const DAY_S: f64 = 86_400.0;
pub const JULIAN_YEAR_S: f64 = 365.25 * DAY_S; // 31_557_600

/// TT − UTC as of 2026 (37 leap seconds + 32.184 s). See module docs.
pub const TT_MINUS_UTC_S: f64 = 69.184;

/// J2000 epoch, JD TDB — `t = 0`.
pub const J2000_JD_TDB: f64 = crate::catalog::J2000_JD_TDB;

/// Rev B default start epoch: 2026-01-01 12:00:00 TDB (JD 2461042.0).
pub const DEFAULT_START_EPOCH_JD_TDB: f64 = 2_461_042.0;

/// Soft range (Rev B §5): 1800-01-01T00:00:00 … 2300-12-31T23:59:59 TDB.
/// Literals are cross-checked against the calendar functions in tests.
pub const T_MIN_S: f64 = -6_311_390_400.0; // JD 2378496.5
pub const T_MAX_S: f64 = 9_498_599_999.0; // JD 2561482.5 − 1 s

/// High-confidence boundary: positions past 2051-01-01T00:00 TDB (or before
/// 1800) are "approximate/extrapolated" and get the §8 toast.
pub const T_HIGH_CONFIDENCE_MAX_S: f64 = 1_609_416_000.0; // JD 2470172.5

/// LIVE chip tolerance: |t − now| below this at +REAL while playing.
pub const LIVE_EPSILON_S: f64 = 2.0;

/// Snap-to-LIVE ease time constant (exponential approach).
const SNAP_TAU_S: f64 = 0.12;
/// Snap completes when within this of the (moving) live target.
const SNAP_DONE_S: f64 = 1.0;

// ---------------------------------------------------------------------------
// Rate ladder
// ---------------------------------------------------------------------------

/// Ladder magnitudes, slow → fast. Signed `RateIndex` selects direction.
/// Months are mean months (Julian year / 12) — calendar months would make the
/// slider's step ratios wobble for no visual gain.
pub const LADDER: [RateStep; 12] = [
    RateStep {
        seconds_per_second: 1.0,
        label: "REAL RATE",
    },
    RateStep {
        seconds_per_second: 60.0,
        label: "1 MIN/S",
    },
    RateStep {
        seconds_per_second: 3_600.0,
        label: "1 HR/S",
    },
    RateStep {
        seconds_per_second: DAY_S,
        label: "1 DAY/S",
    },
    RateStep {
        seconds_per_second: 7.0 * DAY_S,
        label: "1 WK/S",
    },
    RateStep {
        seconds_per_second: JULIAN_YEAR_S / 12.0,
        label: "1 MTH/S",
    },
    RateStep {
        seconds_per_second: JULIAN_YEAR_S / 2.0,
        label: "6 MTHS/S",
    },
    RateStep {
        seconds_per_second: JULIAN_YEAR_S,
        label: "1 YR/S",
    },
    RateStep {
        seconds_per_second: 3.0 * JULIAN_YEAR_S,
        label: "3 YRS/S",
    },
    RateStep {
        seconds_per_second: 10.0 * JULIAN_YEAR_S,
        label: "10 YRS/S",
    },
    RateStep {
        seconds_per_second: 30.0 * JULIAN_YEAR_S,
        label: "30 YRS/S",
    },
    RateStep {
        seconds_per_second: 100.0 * JULIAN_YEAR_S,
        label: "100 YRS/S",
    },
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateStep {
    pub seconds_per_second: f64,
    pub label: &'static str,
}

/// Signed ladder position: `+1` = +REAL … `+12` = +100 yr/s, `−1` = −REAL …
/// `−12` = −100 yr/s. Zero is not a value — the ladder has no "stopped" step
/// (that's what `playing = false` is for), which keeps the slider's center
/// detent unambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RateIndex(i8);

impl RateIndex {
    pub const REAL: RateIndex = RateIndex(1);
    pub const MIN: RateIndex = RateIndex(-12);
    pub const MAX: RateIndex = RateIndex(12);

    pub fn new(i: i8) -> Option<RateIndex> {
        if i != 0 && (-12..=12).contains(&i) {
            Some(RateIndex(i))
        } else {
            None
        }
    }

    pub fn get(self) -> i8 {
        self.0
    }

    pub fn magnitude(self) -> RateStep {
        LADDER[(self.0.unsigned_abs() as usize) - 1]
    }

    /// Signed simulation-seconds per wall-second.
    pub fn seconds_per_second(self) -> f64 {
        self.0.signum() as f64 * self.magnitude().seconds_per_second
    }

    /// Eyes label convention: "REAL RATE", "6 MTHS/S", "−3 YRS/S".
    pub fn label(self) -> String {
        if self.0 < 0 {
            format!("−{}", self.magnitude().label)
        } else {
            self.magnitude().label.to_string()
        }
    }

    /// Step along the ladder, skipping the nonexistent zero (+1 → −1 goes
    /// straight from +REAL to −REAL). Saturates at ±100 yr/s.
    pub fn stepped(self, delta: i8) -> RateIndex {
        let mut i = self.0 as i32 + delta as i32;
        // crossing zero costs no extra step
        if self.0 > 0 && i <= 0 {
            i -= 1;
        } else if self.0 < 0 && i >= 0 {
            i += 1;
        }
        RateIndex(i.clamp(-12, 12) as i8)
    }

    // -- symmetric-log slider mapping (detents at every step, center = REAL) --

    /// Slider position in [−1, 1]. Ladder steps are near-multiplicative, so
    /// uniform detent spacing *is* the symmetric-log layout.
    pub fn slider_pos(self) -> f32 {
        self.0 as f32 / 12.0
    }

    /// Nearest detent for a raw slider position (drag → `SimCommand::SetRate`).
    pub fn from_slider_pos(p: f32) -> RateIndex {
        let i = (p.clamp(-1.0, 1.0) * 12.0).round() as i8;
        RateIndex::new(if i == 0 {
            if p < 0.0 {
                -1
            } else {
                1
            }
        } else {
            i
        })
        .unwrap()
    }

    /// All 24 detents, left to right — the WP8 slider builds from this.
    pub fn detents() -> impl Iterator<Item = RateIndex> {
        (-12..=12).filter(|&i| i != 0).map(RateIndex)
    }
}

impl Default for RateIndex {
    fn default() -> Self {
        RateIndex::REAL
    }
}

// ---------------------------------------------------------------------------
// Calendar (proleptic Gregorian, TDB) — Hinnant's civil algorithms
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

pub fn is_leap_year(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn days_in_month(y: i32, m: u8) -> u8 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Days since 1970-01-01 (can be negative).
fn days_from_civil(y: i32, m: u8, d: u8) -> i64 {
    let y = y as i64 - i64::from(m <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m as u64 - 3 } else { m as u64 + 9 };
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

fn civil_from_days(z: i64) -> (i32, u8, u8) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u8;
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

const UNIX_EPOCH_JD: f64 = 2_440_587.5;

/// Civil TDB datetime → seconds since J2000 (TDB). Errors on impossible
/// dates/times — this is the strict half of WP8's "invalid input reverts".
pub fn t_from_datetime(dt: &DateTime) -> Result<f64, String> {
    if dt.month == 0 || dt.month > 12 {
        return Err(format!("month {} out of range", dt.month));
    }
    if dt.day == 0 || dt.day > days_in_month(dt.year, dt.month) {
        return Err(format!(
            "day {} invalid for {}-{:02}",
            dt.day, dt.year, dt.month
        ));
    }
    if dt.hour > 23 || dt.minute > 59 || dt.second > 59 {
        return Err("time component out of range".into());
    }
    // Exact integer-second path: a fractional-JD detour multiplies rounding
    // error to ~10 µs, which is enough to break floor()-based display.
    let days = days_from_civil(dt.year, dt.month, dt.day);
    let sod = dt.hour as i64 * 3600 + dt.minute as i64 * 60 + dt.second as i64;
    Ok((days * 86_400 + sod) as f64 - SECONDS_J2000_MINUS_UNIX)
}

/// (J2000_JD_TDB − UNIX_EPOCH_JD) · 86400 = 10957.5 d — exactly representable.
/// Note the .5: J2000 is 2000-01-01 *noon*, not the midnight Unix milestone.
const SECONDS_J2000_MINUS_UNIX: f64 = (J2000_JD_TDB - UNIX_EPOCH_JD) * DAY_S;

/// Seconds since J2000 (TDB) → civil TDB datetime (truncated to the second).
pub fn datetime_from_t(t: f64) -> DateTime {
    let unix = SECONDS_J2000_MINUS_UNIX + t;
    let secs = unix.floor() as i64;
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    DateTime {
        year,
        month,
        day,
        hour: (sod / 3600) as u8,
        minute: ((sod % 3600) / 60) as u8,
        second: (sod % 60) as u8,
    }
}

pub fn jd_tdb_from_t(t: f64) -> f64 {
    J2000_JD_TDB + t / DAY_S
}

pub fn t_from_jd_tdb(jd: f64) -> f64 {
    (jd - J2000_JD_TDB) * DAY_S
}

/// Unix (UTC) seconds → clock time (TDB seconds since J2000). See module docs
/// for the constant-offset approximation.
pub fn t_from_unix_utc(unix_s: f64) -> f64 {
    unix_s - SECONDS_J2000_MINUS_UNIX + TT_MINUS_UTC_S
}

/// Strict "YYYY-MM-DD" parser (WP8 editable date field).
pub fn parse_date(s: &str) -> Result<(i32, u8, u8), String> {
    let b: Vec<&str> = s.split('-').collect();
    // A leading '-' (negative year) is out of range anyway; reject via parse.
    if b.len() != 3 {
        return Err("expected YYYY-MM-DD".into());
    }
    let y: i32 = b[0].parse().map_err(|_| "bad year")?;
    let m: u8 = b[1].parse().map_err(|_| "bad month")?;
    let d: u8 = b[2].parse().map_err(|_| "bad day")?;
    if b[1].len() != 2 || b[2].len() != 2 {
        return Err("expected zero-padded YYYY-MM-DD".into());
    }
    Ok((y, m, d))
}

/// Strict "HH:MM:SS" / "HH:MM" parser (WP8 editable clock field).
pub fn parse_time(s: &str) -> Result<(u8, u8, u8), String> {
    let b: Vec<&str> = s.split(':').collect();
    if b.len() != 2 && b.len() != 3 {
        return Err("expected HH:MM[:SS]".into());
    }
    if b.iter().any(|p| p.len() != 2) {
        return Err("expected zero-padded HH:MM[:SS]".into());
    }
    let h: u8 = b[0].parse().map_err(|_| "bad hour")?;
    let m: u8 = b[1].parse().map_err(|_| "bad minute")?;
    let s: u8 = if b.len() == 3 {
        b[2].parse().map_err(|_| "bad second")?
    } else {
        0
    };
    Ok((h, m, s))
}

pub const MONTH_ABBREV: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// Eyes-style date label: "JAN 01, 2026".
pub fn format_date_eyes(dt: &DateTime) -> String {
    format!(
        "{} {:02}, {}",
        MONTH_ABBREV[(dt.month - 1) as usize],
        dt.day,
        dt.year
    )
}

// ---------------------------------------------------------------------------
// SimClock
// ---------------------------------------------------------------------------

/// Startup behavior (Rev B §5): fixed epoch by default, wall clock via the
/// `start_live` setting. Serializable so WP14's settings screen stores it
/// through the Bevy settings framework untouched.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum StartMode {
    FixedEpoch { jd_tdb: f64 },
    Live,
}

impl Default for StartMode {
    fn default() -> Self {
        StartMode::FixedEpoch {
            jd_tdb: DEFAULT_START_EPOCH_JD_TDB,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeEdge {
    AtMin,
    AtMax,
}

/// Per-frame report. UI edge-detects nothing here — transitions only, so a
/// toast fires once per event, not per frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TickReport {
    /// Just hit a range edge this frame (toast: "range clamped at 1800/2300").
    pub clamped: Option<RangeEdge>,
    /// Snap-to-LIVE finished this frame.
    pub snapped_live: bool,
    /// Crossed the high-confidence boundary this frame; payload = now outside
    /// (toast: "positions are extrapolated approximations").
    pub extrapolation_changed: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct SimClock {
    t: f64,
    rate: RateIndex,
    playing: bool,
    snapping: bool,
    was_outside_confidence: bool,
    at_edge: Option<RangeEdge>,
}

impl SimClock {
    /// `wall_now_t` is `t_from_unix_utc(system time)`, supplied by the caller
    /// so the core stays deterministic and replay-testable.
    pub fn new(start: StartMode, wall_now_t: f64) -> SimClock {
        let t = match start {
            StartMode::FixedEpoch { jd_tdb } => t_from_jd_tdb(jd_tdb),
            StartMode::Live => wall_now_t,
        }
        .clamp(T_MIN_S, T_MAX_S);
        SimClock {
            t,
            rate: RateIndex::REAL,
            playing: true,
            snapping: false,
            was_outside_confidence: !in_high_confidence_range(t),
            at_edge: None,
        }
    }

    // -- queries ------------------------------------------------------------

    pub fn t(&self) -> f64 {
        self.t
    }
    pub fn jd_tdb(&self) -> f64 {
        jd_tdb_from_t(self.t)
    }
    pub fn datetime(&self) -> DateTime {
        datetime_from_t(self.t)
    }
    pub fn rate(&self) -> RateIndex {
        self.rate
    }
    pub fn is_playing(&self) -> bool {
        self.playing
    }
    pub fn is_snapping(&self) -> bool {
        self.snapping
    }

    /// LIVE chip state: green iff tracking the wall clock at +REAL.
    pub fn is_live(&self, wall_now_t: f64) -> bool {
        self.playing
            && !self.snapping
            && self.rate == RateIndex::REAL
            && (self.t - wall_now_t).abs() < LIVE_EPSILON_S
    }

    // -- commands (each is one SimCommand arm in the app) ---------------------

    pub fn play(&mut self) {
        self.playing = true;
    }
    pub fn pause(&mut self) {
        self.playing = false;
        self.snapping = false;
    }
    pub fn toggle_play(&mut self) {
        if self.playing {
            self.pause()
        } else {
            self.play()
        }
    }

    pub fn set_rate(&mut self, rate: RateIndex) {
        self.rate = rate;
        self.snapping = false;
        self.at_edge = None; // a new rate may move off the edge
    }

    pub fn step_rate(&mut self, delta: i8) {
        self.set_rate(self.rate.stepped(delta));
    }

    /// Jump to an absolute time, clamped to the soft range. Reports whether
    /// clamping occurred so WP8 can show the range toast on typed dates too.
    pub fn set_t(&mut self, t: f64) -> Option<RangeEdge> {
        self.snapping = false;
        let clamped = if t < T_MIN_S {
            Some(RangeEdge::AtMin)
        } else if t > T_MAX_S {
            Some(RangeEdge::AtMax)
        } else {
            None
        };
        self.t = t.clamp(T_MIN_S, T_MAX_S);
        self.at_edge = clamped;
        clamped
    }

    /// Typed date/time (WP8): strict validation, then jump.
    pub fn set_datetime(&mut self, dt: &DateTime) -> Result<Option<RangeEdge>, String> {
        let t = t_from_datetime(dt)?;
        Ok(self.set_t(t))
    }

    /// Begin the eased snap to the wall clock (LIVE chip click).
    pub fn snap_to_live(&mut self) {
        self.snapping = true;
        self.playing = true;
    }

    // -- per-frame ------------------------------------------------------------

    pub fn tick(&mut self, wall_dt_s: f64, wall_now_t: f64) -> TickReport {
        let mut report = TickReport::default();

        if self.snapping {
            // Exponential approach to a moving target — the classic eased snap.
            let alpha = 1.0 - (-wall_dt_s / SNAP_TAU_S).exp();
            self.t += (wall_now_t - self.t) * alpha;
            if (wall_now_t - self.t).abs() < SNAP_DONE_S {
                self.t = wall_now_t.clamp(T_MIN_S, T_MAX_S);
                self.snapping = false;
                self.rate = RateIndex::REAL;
                self.playing = true;
                report.snapped_live = true;
            }
        } else if self.playing {
            self.t += self.rate.seconds_per_second() * wall_dt_s;
        }

        // Soft-range clamp, transition-edge reporting (pinned, not paused —
        // reversing the rate walks straight back off the edge).
        let edge = if self.t <= T_MIN_S {
            self.t = T_MIN_S;
            Some(RangeEdge::AtMin)
        } else if self.t >= T_MAX_S {
            self.t = T_MAX_S;
            Some(RangeEdge::AtMax)
        } else {
            None
        };
        if edge != self.at_edge {
            report.clamped = edge;
            self.at_edge = edge;
        }

        let outside = !in_high_confidence_range(self.t);
        if outside != self.was_outside_confidence {
            report.extrapolation_changed = Some(outside);
            self.was_outside_confidence = outside;
        }

        report
    }
}

/// Inside 1800–2050 (Rev B: outside it, show the "approximate positions" note).
pub fn in_high_confidence_range(t: f64) -> bool {
    (T_MIN_S..=T_HIGH_CONFIDENCE_MAX_S).contains(&t)
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- ladder math ----

    #[test]
    fn ladder_magnitudes_and_signs() {
        assert_eq!(RateIndex::REAL.seconds_per_second(), 1.0);
        assert_eq!(RateIndex::new(-1).unwrap().seconds_per_second(), -1.0);
        assert_eq!(RateIndex::new(4).unwrap().seconds_per_second(), DAY_S);
        assert_eq!(
            RateIndex::new(12).unwrap().seconds_per_second(),
            100.0 * JULIAN_YEAR_S
        );
        assert_eq!(
            RateIndex::new(-12).unwrap().seconds_per_second(),
            -100.0 * JULIAN_YEAR_S
        );
        assert_eq!(
            RateIndex::new(7).unwrap().seconds_per_second(),
            JULIAN_YEAR_S / 2.0
        );
    }

    #[test]
    fn ladder_labels_follow_eyes_convention() {
        assert_eq!(RateIndex::REAL.label(), "REAL RATE");
        assert_eq!(RateIndex::new(7).unwrap().label(), "6 MTHS/S");
        assert_eq!(RateIndex::new(-9).unwrap().label(), "−3 YRS/S");
        assert_eq!(RateIndex::new(12).unwrap().label(), "100 YRS/S");
    }

    #[test]
    fn rate_index_rejects_zero_and_out_of_range() {
        assert!(RateIndex::new(0).is_none());
        assert!(RateIndex::new(13).is_none());
        assert!(RateIndex::new(-13).is_none());
    }

    #[test]
    fn stepping_skips_zero_and_saturates() {
        assert_eq!(RateIndex::REAL.stepped(-1), RateIndex::new(-1).unwrap());
        assert_eq!(RateIndex::new(-1).unwrap().stepped(1), RateIndex::REAL);
        assert_eq!(
            RateIndex::new(12).unwrap().stepped(1),
            RateIndex::new(12).unwrap()
        );
        assert_eq!(
            RateIndex::new(-12).unwrap().stepped(-3),
            RateIndex::new(-12).unwrap()
        );
        assert_eq!(
            RateIndex::new(2).unwrap().stepped(-3),
            RateIndex::new(-2).unwrap()
        );
    }

    #[test]
    fn slider_round_trips_every_detent() {
        let mut count = 0;
        for idx in RateIndex::detents() {
            assert_eq!(RateIndex::from_slider_pos(idx.slider_pos()), idx);
            count += 1;
        }
        assert_eq!(count, 24);
        // dead-center drag resolves to +REAL; slightly-left to −REAL
        assert_eq!(RateIndex::from_slider_pos(0.0), RateIndex::REAL);
        assert_eq!(
            RateIndex::from_slider_pos(-0.02),
            RateIndex::new(-1).unwrap()
        );
        // extremes clamp
        assert_eq!(RateIndex::from_slider_pos(9.0), RateIndex::new(12).unwrap());
    }

    // ---- calendar and round-trips ----

    #[test]
    fn t_zero_is_j2000_noon() {
        let dt = datetime_from_t(0.0);
        assert_eq!(
            dt,
            DateTime {
                year: 2000,
                month: 1,
                day: 1,
                hour: 12,
                minute: 0,
                second: 0
            }
        );
    }

    #[test]
    fn unix_epoch_jd_constant_is_consistent() {
        // Pins the noon-vs-midnight trap from the WP1 change log:
        // J2000 is JD 2451545.0 (noon); Unix epoch is JD 2440587.5 (midnight).
        let seconds_from_jd = (J2000_JD_TDB - UNIX_EPOCH_JD) * DAY_S;
        assert_eq!(seconds_from_jd, 946_728_000.0);
        assert_eq!(seconds_from_jd, SECONDS_J2000_MINUS_UNIX);
    }

    #[test]
    fn default_epoch_is_2026_jan_1_noon() {
        let t = t_from_jd_tdb(DEFAULT_START_EPOCH_JD_TDB);
        let dt = datetime_from_t(t);
        assert_eq!(
            dt,
            DateTime {
                year: 2026,
                month: 1,
                day: 1,
                hour: 12,
                minute: 0,
                second: 0
            }
        );
    }

    #[test]
    fn range_literals_match_calendar_functions() {
        let min = t_from_datetime(&DateTime {
            year: 1800,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
        })
        .unwrap();
        assert_eq!(min, T_MIN_S);
        let max = t_from_datetime(&DateTime {
            year: 2300,
            month: 12,
            day: 31,
            hour: 23,
            minute: 59,
            second: 59,
        })
        .unwrap();
        assert_eq!(max, T_MAX_S);
        let conf = t_from_datetime(&DateTime {
            year: 2051,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
        })
        .unwrap();
        assert_eq!(conf, T_HIGH_CONFIDENCE_MAX_S + 1.0 - 1.0); // exact boundary
        assert!(in_high_confidence_range(conf - 1.0));
        assert!(in_high_confidence_range(conf)); // inclusive boundary
        assert!(!in_high_confidence_range(conf + 1.0));
    }

    #[test]
    fn date_round_trip_across_the_range_and_leap_rules() {
        let cases = [
            (1800, 1, 1, 0, 0, 0),
            (1899, 12, 31, 23, 59, 59),
            (1900, 2, 28, 12, 0, 0), // 1900 not leap (century rule)
            (1904, 2, 29, 6, 30, 15),
            (1986, 2, 9, 15, 0, 0), // Halley perihelion demo stop
            (2000, 2, 29, 0, 0, 1), // 2000 leap (400 rule)
            (2024, 2, 29, 23, 0, 0),
            (2026, 1, 1, 12, 0, 0),
            (2100, 2, 28, 1, 2, 3), // 2100 not leap
            (2299, 6, 15, 18, 45, 59),
            (2300, 12, 31, 23, 59, 59),
        ];
        for (y, mo, d, h, mi, s) in cases {
            let dt = DateTime {
                year: y,
                month: mo,
                day: d,
                hour: h,
                minute: mi,
                second: s,
            };
            let t = t_from_datetime(&dt).unwrap();
            assert_eq!(
                datetime_from_t(t),
                dt,
                "round-trip failed for {y}-{mo:02}-{d:02}"
            );
        }
    }

    #[test]
    fn invalid_dates_are_rejected() {
        let bad = [
            DateTime {
                year: 1900,
                month: 2,
                day: 29,
                hour: 0,
                minute: 0,
                second: 0,
            },
            DateTime {
                year: 2026,
                month: 13,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0,
            },
            DateTime {
                year: 2026,
                month: 4,
                day: 31,
                hour: 0,
                minute: 0,
                second: 0,
            },
            DateTime {
                year: 2026,
                month: 0,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0,
            },
            DateTime {
                year: 2026,
                month: 1,
                day: 1,
                hour: 24,
                minute: 0,
                second: 0,
            },
            DateTime {
                year: 2026,
                month: 1,
                day: 1,
                hour: 0,
                minute: 60,
                second: 0,
            },
        ];
        for dt in bad {
            assert!(t_from_datetime(&dt).is_err(), "{dt:?} should be invalid");
        }
    }

    #[test]
    fn strict_parsers() {
        assert_eq!(parse_date("2026-01-01").unwrap(), (2026, 1, 1));
        assert!(parse_date("2026-1-1").is_err());
        assert!(parse_date("01/01/2026").is_err());
        assert!(parse_date("hello").is_err());
        assert_eq!(parse_time("09:16:36").unwrap(), (9, 16, 36));
        assert_eq!(parse_time("09:16").unwrap(), (9, 16, 0));
        assert!(parse_time("9:16").is_err());
        assert!(parse_time("09:16:36:00").is_err());
    }

    #[test]
    fn eyes_date_label() {
        let dt = DateTime {
            year: 2026,
            month: 7,
            day: 11,
            hour: 21,
            minute: 16,
            second: 36,
        };
        assert_eq!(format_date_eyes(&dt), "JUL 11, 2026");
    }

    #[test]
    fn unix_conversion_applies_tt_offset() {
        // Unix 0 = 1970-01-01T00:00:00 UTC → TDB is ~69.184 s later
        let t = t_from_unix_utc(0.0);
        let dt = datetime_from_t(t);
        assert_eq!(
            (dt.year, dt.month, dt.day, dt.hour, dt.minute),
            (1970, 1, 1, 0, 1)
        );
        assert_eq!(dt.second, 9); // 69.184 s → 00:01:09 TDB
    }

    // ---- clock behavior ----

    const NOW: f64 = 8.0e8; // arbitrary wall-clock t within range

    fn fixed_clock() -> SimClock {
        SimClock::new(StartMode::default(), NOW)
    }

    #[test]
    fn start_modes() {
        let c = fixed_clock();
        assert_eq!(c.jd_tdb(), DEFAULT_START_EPOCH_JD_TDB);
        assert!(c.is_playing());
        assert_eq!(c.rate(), RateIndex::REAL);

        let live = SimClock::new(StartMode::Live, NOW);
        assert_eq!(live.t(), NOW);
        assert!(live.is_live(NOW));
    }

    #[test]
    fn advancing_at_ladder_rates() {
        let mut c = fixed_clock();
        let t0 = c.t();
        c.set_rate(RateIndex::new(8).unwrap()); // 1 yr/s
        c.tick(2.0, NOW);
        assert!((c.t() - t0 - 2.0 * JULIAN_YEAR_S).abs() < 1e-6);
        c.set_rate(RateIndex::new(-4).unwrap()); // −1 day/s
        c.tick(0.5, NOW);
        assert!((c.t() - t0 - 2.0 * JULIAN_YEAR_S + 0.5 * DAY_S).abs() < 1e-6);
    }

    #[test]
    fn pause_freezes_time() {
        let mut c = fixed_clock();
        c.pause();
        let t0 = c.t();
        c.tick(10.0, NOW);
        assert_eq!(c.t(), t0);
        c.toggle_play();
        c.tick(1.0, NOW);
        assert!((c.t() - t0 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn live_detection_requires_real_rate_playing_and_proximity() {
        let mut c = SimClock::new(StartMode::Live, NOW);
        assert!(c.is_live(NOW));
        assert!(c.is_live(NOW + LIVE_EPSILON_S * 0.9));
        assert!(!c.is_live(NOW + LIVE_EPSILON_S * 1.1));
        c.pause();
        assert!(!c.is_live(NOW));
        c.play();
        c.set_rate(RateIndex::new(2).unwrap());
        assert!(!c.is_live(NOW));
    }

    #[test]
    fn snap_to_live_eases_and_lands() {
        let mut c = fixed_clock();
        c.set_rate(RateIndex::new(-9).unwrap());
        c.snap_to_live();
        assert!(c.is_snapping());
        let mut landed = false;
        let mut prev_gap = (NOW - c.t()).abs();
        for frame in 0..600 {
            let r = c.tick(1.0 / 60.0, NOW);
            let gap = (NOW - c.t()).abs();
            assert!(gap <= prev_gap + 1e-9, "gap must shrink monotonically");
            prev_gap = gap;
            if r.snapped_live {
                landed = true;
                assert!(frame > 5, "a multi-year gap must not land instantly");
                break;
            }
        }
        assert!(landed, "snap never completed");
        assert_eq!(c.t(), NOW);
        assert_eq!(c.rate(), RateIndex::REAL);
        assert!(c.is_live(NOW));
    }

    #[test]
    fn clamps_at_range_edges_report_once_and_pin() {
        let mut c = fixed_clock();
        // 2026 → past 2300 in under 3 wall-seconds.
        c.set_rate(RateIndex::MAX); // +100 yr/s
        let r1 = c.tick(5.0, NOW);
        assert_eq!(r1.clamped, Some(RangeEdge::AtMax));
        assert_eq!(c.t(), T_MAX_S);
        let r2 = c.tick(5.0, NOW);
        assert_eq!(r2.clamped, None, "edge reported once, not per frame");
        assert_eq!(c.t(), T_MAX_S, "pinned while pushing outward");
        // reverse: walks back off the edge
        c.set_rate(RateIndex::MIN);
        c.tick(1.0, NOW);
        assert!(c.t() < T_MAX_S);
        // all the way down to 1800
        let mut hit_min = false;
        for _ in 0..20 {
            if c.tick(1.0, NOW).clamped == Some(RangeEdge::AtMin) {
                hit_min = true;
                break;
            }
        }
        assert!(hit_min);
        assert_eq!(c.t(), T_MIN_S);
    }

    #[test]
    fn set_t_and_typed_dates_clamp_and_report() {
        let mut c = fixed_clock();
        assert_eq!(c.set_t(T_MAX_S + 1.0e9), Some(RangeEdge::AtMax));
        assert_eq!(c.t(), T_MAX_S);
        assert_eq!(c.set_t(0.0), None);

        let ok = DateTime {
            year: 1986,
            month: 2,
            day: 9,
            hour: 0,
            minute: 0,
            second: 0,
        };
        assert_eq!(c.set_datetime(&ok).unwrap(), None);
        assert_eq!(c.datetime(), ok);

        let invalid = DateTime {
            year: 1900,
            month: 2,
            day: 29,
            hour: 0,
            minute: 0,
            second: 0,
        };
        assert!(c.set_datetime(&invalid).is_err());
        assert_eq!(c.datetime(), ok, "invalid input must not move the clock");
    }

    #[test]
    fn extrapolation_boundary_events_fire_on_transitions_only() {
        let mut c = fixed_clock(); // 2026: inside high-confidence range
        c.set_rate(RateIndex::new(10).unwrap()); // 10 yr/s
        let mut entered = 0;
        let mut exited = 0;
        for _ in 0..10 {
            match c.tick(1.0, NOW).extrapolation_changed {
                Some(true) => entered += 1,
                Some(false) => exited += 1,
                None => {}
            }
        }
        assert_eq!((entered, exited), (1, 0), "one entry event, no spam");
        c.set_rate(RateIndex::new(-10).unwrap());
        for _ in 0..10 {
            match c.tick(1.0, NOW).extrapolation_changed {
                Some(false) => exited += 1,
                Some(true) => entered += 1,
                None => {}
            }
        }
        assert_eq!((entered, exited), (1, 1));
    }
}
