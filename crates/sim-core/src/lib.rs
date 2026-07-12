//! `sim-core` — engine-agnostic simulation core (Rev B §7).
//!
//! Dependency firewall: this crate contains zero Bevy (or any engine) types.
//! It is shared between the Rev B desktop product and the V1 custom-core track.
//!
//! Modules land in WP order:
//! - `catalog` (WP3, this deliverable): `catalog.ron` schema + load-time validation
//! - `time`    (WP1): SimClock, full ±100 yr/s ladder, start epoch, LIVE, 1800–2300 range
//! - `kepler`  (WP2): elliptic + hyperbolic solvers

pub mod catalog;
pub mod kepler;
pub mod time;
