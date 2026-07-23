//! Rev E Q24 orbit-palette perceptual audit and deterministic review report.
//!
//! The manifest owns the reviewed RGB values. This module converts sRGB to
//! CIE Lab, enforces the delegated ΔE76 gates, and reports contrast plus
//! color-vision-simulation convergence without adding a color-math dependency.

use crate::manifest::Entry;
use sim_core::catalog::Category;
use std::collections::HashMap;

pub const MIN_ALL_BODIES_DELTA_E76: f64 = 4.0;
pub const MIN_PLANET_DELTA_E76: f64 = 25.0;

const PROTANOPIA: [[f64; 3]; 3] = [
    [0.152_286, 1.052_583, -0.204_868],
    [0.114_503, 0.786_281, 0.099_216],
    [-0.003_882, -0.048_116, 1.051_998],
];
const DEUTERANOPIA: [[f64; 3]; 3] = [
    [0.367_322, 0.860_646, -0.227_968],
    [0.280_085, 0.672_501, 0.047_413],
    [-0.011_820, 0.042_940, 0.968_881],
];
const TRITANOPIA: [[f64; 3]; 3] = [
    [1.255_528, -0.076_749, -0.178_779],
    [-0.078_411, 0.930_809, 0.147_602],
    [0.004_733, 0.691_367, 0.303_900],
];

#[derive(Debug, Clone, PartialEq)]
pub struct PairDistance {
    pub first_id: String,
    pub second_id: String,
    pub delta_e76: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaletteAudit {
    pub minimum_pair: PairDistance,
    pub minimum_planet_pair: PairDistance,
    pub minimum_protanopia_pair: PairDistance,
    pub minimum_deuteranopia_pair: PairDistance,
    pub minimum_tritanopia_pair: PairDistance,
    pub minimum_contrast_id: String,
    pub minimum_contrast_ratio: f64,
}

fn srgb_channel_to_linear(channel: u8) -> f64 {
    let value = f64::from(channel) / 255.0;
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_rgb(rgb: (u8, u8, u8)) -> [f64; 3] {
    [
        srgb_channel_to_linear(rgb.0),
        srgb_channel_to_linear(rgb.1),
        srgb_channel_to_linear(rgb.2),
    ]
}

fn linear_rgb_to_lab(rgb: [f64; 3]) -> [f64; 3] {
    let x = (0.412_456_4 * rgb[0] + 0.357_576_1 * rgb[1] + 0.180_437_5 * rgb[2]) / 0.950_47;
    let y = 0.212_672_9 * rgb[0] + 0.715_152_2 * rgb[1] + 0.072_175 * rgb[2];
    let z = (0.019_333_9 * rgb[0] + 0.119_192 * rgb[1] + 0.950_304_1 * rgb[2]) / 1.088_83;
    let delta = 6.0 / 29.0;
    let convert = |value: f64| {
        if value > delta * delta * delta {
            value.cbrt()
        } else {
            value / (3.0 * delta * delta) + 4.0 / 29.0
        }
    };
    let fx = convert(x);
    let fy = convert(y);
    let fz = convert(z);
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

pub fn cie76(rgb_a: (u8, u8, u8), rgb_b: (u8, u8, u8)) -> f64 {
    let a = linear_rgb_to_lab(linear_rgb(rgb_a));
    let b = linear_rgb_to_lab(linear_rgb(rgb_b));
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn simulated_lab(rgb: (u8, u8, u8), matrix: [[f64; 3]; 3]) -> [f64; 3] {
    let rgb = linear_rgb(rgb);
    let transformed =
        matrix.map(|row| (row[0] * rgb[0] + row[1] * rgb[1] + row[2] * rgb[2]).clamp(0.0, 1.0));
    linear_rgb_to_lab(transformed)
}

fn lab_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn contrast_against_black(rgb: (u8, u8, u8)) -> f64 {
    let rgb = linear_rgb(rgb);
    let luminance = 0.212_6 * rgb[0] + 0.715_2 * rgb[1] + 0.072_2 * rgb[2];
    (luminance + 0.05) / 0.05
}

fn find_minimum_pair(
    entries: &[&Entry],
    distance: impl Fn((u8, u8, u8), (u8, u8, u8)) -> f64,
) -> PairDistance {
    let mut minimum = PairDistance {
        first_id: String::new(),
        second_id: String::new(),
        delta_e76: f64::INFINITY,
    };
    for (index, first) in entries.iter().enumerate() {
        for second in entries.iter().skip(index + 1) {
            let delta_e76 = distance(first.orbit_color, second.orbit_color);
            if delta_e76 < minimum.delta_e76 {
                minimum = PairDistance {
                    first_id: first.id.to_string(),
                    second_id: second.id.to_string(),
                    delta_e76,
                };
            }
        }
    }
    minimum
}

pub fn audit(entries: &[Entry]) -> Result<PaletteAudit, String> {
    let orbiting = entries
        .iter()
        .filter(|entry| entry.category != Category::Star)
        .collect::<Vec<_>>();
    if orbiting.len() != 65 {
        return Err(format!(
            "expected 65 orbiting manifest bodies, found {}",
            orbiting.len()
        ));
    }
    let mut colors = HashMap::new();
    for entry in &orbiting {
        if let Some(first) = colors.insert(entry.orbit_color, entry.id) {
            return Err(format!(
                "duplicate orbit RGB {:?}: {first} and {}",
                entry.orbit_color, entry.id
            ));
        }
    }
    let planets = orbiting
        .iter()
        .copied()
        .filter(|entry| entry.category == Category::Planet)
        .collect::<Vec<_>>();
    let minimum_pair = find_minimum_pair(&orbiting, cie76);
    let minimum_planet_pair = find_minimum_pair(&planets, cie76);
    let mut violations = Vec::new();
    for (index, first) in orbiting.iter().enumerate() {
        for second in orbiting.iter().skip(index + 1) {
            let delta = cie76(first.orbit_color, second.orbit_color);
            if delta + f64::EPSILON < MIN_ALL_BODIES_DELTA_E76 {
                violations.push(format!("{}–{}={delta:.3}", first.id, second.id));
            }
        }
    }
    if !violations.is_empty() {
        return Err(format!(
            "all-body CIE76 ΔE violations below {:.1}: {}",
            MIN_ALL_BODIES_DELTA_E76,
            violations.join(", ")
        ));
    }
    let mut planet_violations = Vec::new();
    for (index, first) in planets.iter().enumerate() {
        for second in planets.iter().skip(index + 1) {
            let delta = cie76(first.orbit_color, second.orbit_color);
            if delta + f64::EPSILON < MIN_PLANET_DELTA_E76 {
                planet_violations.push(format!("{}–{}={delta:.3}", first.id, second.id));
            }
        }
    }
    if !planet_violations.is_empty() {
        return Err(format!(
            "planet CIE76 ΔE violations below {:.1}: {}",
            MIN_PLANET_DELTA_E76,
            planet_violations.join(", ")
        ));
    }
    let simulated_minimum = |matrix| {
        find_minimum_pair(&orbiting, |a, b| {
            lab_distance(simulated_lab(a, matrix), simulated_lab(b, matrix))
        })
    };
    let (minimum_contrast_id, minimum_contrast_ratio) = orbiting
        .iter()
        .map(|entry| {
            (
                entry.id.to_string(),
                contrast_against_black(entry.orbit_color),
            )
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("65-body manifest is non-empty");
    Ok(PaletteAudit {
        minimum_pair,
        minimum_planet_pair,
        minimum_protanopia_pair: simulated_minimum(PROTANOPIA),
        minimum_deuteranopia_pair: simulated_minimum(DEUTERANOPIA),
        minimum_tritanopia_pair: simulated_minimum(TRITANOPIA),
        minimum_contrast_id,
        minimum_contrast_ratio,
    })
}

pub fn format_review_report(entries: &[Entry]) -> Result<String, String> {
    let audit = audit(entries)?;
    let orbiting = entries
        .iter()
        .filter(|entry| entry.category != Category::Star)
        .collect::<Vec<_>>();
    let mut report = String::from(
        "# Orbit palette perceptual review (2026-07-23)\n\n\
Generated from `xtask/src/manifest.rs` by `cargo run -p xtask -- \
orbit-palette-report --out docs/orbit-palette-review-2026-07-23.md`.\n\n\
The binding normal-vision gates are exact 65/65 RGB uniqueness, CIE76 ΔE ≥ 4 \
for every pair, and CIE76 ΔE ≥ 25 between planets. Simulated convergence is \
review input rather than a failure: category width and accessible body labels \
remain the primary redundant cues.\n\n",
    );
    report.push_str(&format!(
        "- Minimum all-body CIE76: **{:.2}** (`{}` / `{}`)\n\
- Minimum planet CIE76: **{:.2}** (`{}` / `{}`)\n\
- Lowest base-color contrast against black: **{:.2}:1** (`{}`)\n\
- Protanopia simulated minimum: **{:.2}** (`{}` / `{}`)\n\
- Deuteranopia simulated minimum: **{:.2}** (`{}` / `{}`)\n\
- Tritanopia simulated minimum: **{:.2}** (`{}` / `{}`)\n\n",
        audit.minimum_pair.delta_e76,
        audit.minimum_pair.first_id,
        audit.minimum_pair.second_id,
        audit.minimum_planet_pair.delta_e76,
        audit.minimum_planet_pair.first_id,
        audit.minimum_planet_pair.second_id,
        audit.minimum_contrast_ratio,
        audit.minimum_contrast_id,
        audit.minimum_protanopia_pair.delta_e76,
        audit.minimum_protanopia_pair.first_id,
        audit.minimum_protanopia_pair.second_id,
        audit.minimum_deuteranopia_pair.delta_e76,
        audit.minimum_deuteranopia_pair.first_id,
        audit.minimum_deuteranopia_pair.second_id,
        audit.minimum_tritanopia_pair.delta_e76,
        audit.minimum_tritanopia_pair.first_id,
        audit.minimum_tritanopia_pair.second_id,
    ));
    report.push_str("| Body | Category | Orbit RGB | Contrast | Nearest normal-vision pair ΔE |\n");
    report.push_str("|---|---|---:|---:|---:|\n");
    for entry in orbiting {
        let nearest = entries
            .iter()
            .filter(|other| other.category != Category::Star && other.id != entry.id)
            .map(|other| (other.id, cie76(entry.orbit_color, other.orbit_color)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .expect("every orbiting body has a peer");
        report.push_str(&format!(
            "| {} | {} | `#{:02X}{:02X}{:02X}` | {:.2}:1 | {} ({:.2}) |\n",
            entry.name,
            entry.category,
            entry.orbit_color.0,
            entry.orbit_color.1,
            entry.orbit_color.2,
            contrast_against_black(entry.orbit_color),
            nearest.0,
            nearest.1,
        ));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewed_manifest_palette_satisfies_exact_cie76_gates() {
        let entries = crate::manifest::entries();
        let audit = audit(&entries).unwrap();
        assert!(audit.minimum_pair.delta_e76 >= MIN_ALL_BODIES_DELTA_E76);
        assert!(audit.minimum_planet_pair.delta_e76 >= MIN_PLANET_DELTA_E76);
    }

    #[test]
    fn report_is_deterministic_and_covers_all_orbiting_bodies() {
        let report = format_review_report(&crate::manifest::entries()).unwrap();
        assert!(report.contains("Minimum all-body CIE76"));
        assert!(report.contains("Protanopia simulated minimum"));
        assert_eq!(
            report.lines().filter(|line| line.starts_with("| ")).count(),
            66
        );
    }
}
