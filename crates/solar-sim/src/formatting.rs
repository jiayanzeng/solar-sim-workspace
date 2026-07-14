//! WP14 presentation-unit formatting — Rev C §8.5.
//!
//! Simulation and catalog truth stay in kilometres. This module is the sole
//! presentation boundary that converts a distance for display, so changing
//! the units setting cannot leave a stale km-only label elsewhere in the UI.

use crate::DistanceUnit;
use sim_core::catalog::AU_KM;

const KM_PER_MILE: f64 = 1.609_344;

/// Formats a kilometre-valued distance in the selected presentation unit.
pub fn format_distance_km(distance_km: f64, unit: DistanceUnit) -> String {
    match unit {
        DistanceUnit::Kilometers => format!("{distance_km:.0} km"),
        DistanceUnit::Miles => format!("{:.0} mi", distance_km / KM_PER_MILE),
        DistanceUnit::AstronomicalUnits => {
            let au = distance_km / AU_KM;
            if au.abs() < 0.01 {
                format!("{au:.6} AU")
            } else {
                format!("{au:.3} AU")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_all_three_unit_modes_from_one_kilometre_source() {
        let earth_radius_km = 6_378.137;
        assert_eq!(
            format_distance_km(earth_radius_km, DistanceUnit::Kilometers),
            "6378 km"
        );
        assert_eq!(
            format_distance_km(earth_radius_km, DistanceUnit::Miles),
            "3963 mi"
        );
        assert_eq!(
            format_distance_km(earth_radius_km, DistanceUnit::AstronomicalUnits),
            "0.000043 AU"
        );
    }

    #[test]
    fn astronomical_unit_formatter_uses_the_catalog_constant() {
        assert_eq!(
            format_distance_km(AU_KM, DistanceUnit::AstronomicalUnits),
            "1.000 AU"
        );
    }
}
