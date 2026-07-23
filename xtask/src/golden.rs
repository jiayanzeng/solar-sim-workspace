//! WP15 golden-screenshot orchestration and perceptual comparison.
//!
//! The renderer writes uncompressed PPM captures so this offline harness can
//! inspect exact pixels without adding an image-codec dependency. Comparison
//! happens in CIE Lab space: a small mean Delta E permits backend noise while
//! the 99th percentile cap still catches localized seams, missing labels, and
//! broken ring/orbit geometry.

use crate::texture::{parse_binary_ppm, RasterImage, TexturePipelineError};
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const CANONICAL_VIEW_SLUGS: [&str; 6] = [
    "full-system",
    "inner-orbits",
    "earth-texture",
    "jupiter-system",
    "saturn-rings",
    "sun-bloom",
];
pub const ORBIT_REVIEW_VIEW_SLUGS: [&str; 10] = [
    "orbit-full-normal",
    "orbit-full-emphasis",
    "orbit-belt-normal",
    "orbit-belt-emphasis",
    "orbit-jupiter-normal",
    "orbit-jupiter-emphasis",
    "orbit-saturn-normal",
    "orbit-saturn-emphasis",
    "orbit-comet-normal",
    "orbit-comet-emphasis",
];
pub const SCALE_REVIEW_VIEW_SLUGS: [&str; 14] = [
    "scale-ceres-floor-x1",
    "scale-ceres-floor-x10",
    "scale-ceres-floor-x50",
    "scale-earth-overview-x1",
    "scale-earth-overview-x10",
    "scale-earth-overview-x50",
    "scale-saturn-rings-x1",
    "scale-saturn-rings-x10",
    "scale-saturn-rings-x50",
    "scale-ceres-close-x1",
    "scale-ceres-close-x10",
    "scale-ceres-close-x50",
    "appearance-pluto",
    "appearance-charon",
];
pub const DEFAULT_MAX_MEAN_DELTA_E: f64 = 1.25;
pub const DEFAULT_MAX_P99_DELTA_E: f64 = 4.0;
pub const ATTEMPTS_MANIFEST_FILE: &str = "golden-attempts.txt";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerceptualThreshold {
    pub max_mean_delta_e: f64,
    pub max_p99_delta_e: f64,
}

impl Default for PerceptualThreshold {
    fn default() -> Self {
        Self {
            max_mean_delta_e: DEFAULT_MAX_MEAN_DELTA_E,
            max_p99_delta_e: DEFAULT_MAX_P99_DELTA_E,
        }
    }
}

impl PerceptualThreshold {
    pub fn validate(self) -> Result<Self, GoldenError> {
        if self.max_mean_delta_e.is_finite()
            && self.max_mean_delta_e >= 0.0
            && self.max_p99_delta_e.is_finite()
            && self.max_p99_delta_e >= 0.0
        {
            Ok(self)
        } else {
            Err(GoldenError::Configuration(
                "perceptual thresholds must be finite and non-negative".into(),
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GoldenComparison {
    pub view: String,
    pub width: u32,
    pub height: u32,
    pub mean_delta_e: f64,
    pub p99_delta_e: f64,
    pub baseline_attempts: u8,
    pub candidate_attempts: u8,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenAttemptCount {
    pub view: String,
    pub attempts: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenCaptureReport {
    pub directory: PathBuf,
    pub attempts: Vec<GoldenAttemptCount>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GoldenError {
    Configuration(String),
    Read {
        path: PathBuf,
        message: String,
    },
    ViewSet {
        directory: PathBuf,
        missing: Vec<String>,
        unexpected: Vec<String>,
    },
    Image {
        view: String,
        message: String,
    },
    Attempts {
        path: PathBuf,
        message: String,
    },
    ThresholdExceeded(Vec<GoldenComparison>),
    RetriesDetected(Vec<GoldenComparison>),
    Launch {
        view: String,
        message: String,
    },
}

impl fmt::Display for GoldenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(message) => write!(f, "invalid golden configuration: {message}"),
            Self::Read { path, message } => {
                write!(f, "could not read '{}': {message}", path.display())
            }
            Self::ViewSet {
                directory,
                missing,
                unexpected,
            } => write!(
                f,
                "golden directory '{}' has the wrong view set (missing: {}; unexpected: {})",
                directory.display(),
                display_names(missing),
                display_names(unexpected)
            ),
            Self::Image { view, message } => {
                write!(f, "golden view '{view}' is invalid: {message}")
            }
            Self::Attempts { path, message } => write!(
                f,
                "golden attempts manifest '{}' is invalid: {message}",
                path.display()
            ),
            Self::ThresholdExceeded(comparisons) => {
                write!(f, "golden perceptual threshold exceeded")?;
                for comparison in comparisons.iter().filter(|comparison| !comparison.passed) {
                    write!(
                        f,
                        "\n- {}: mean Delta E {:.4}, p99 Delta E {:.4}",
                        comparison.view, comparison.mean_delta_e, comparison.p99_delta_e
                    )?;
                }
                Ok(())
            }
            Self::RetriesDetected(comparisons) => {
                write!(f, "golden capture retry detected")?;
                for comparison in comparisons.iter().filter(|comparison| {
                    comparison.baseline_attempts > 1 || comparison.candidate_attempts > 1
                }) {
                    write!(
                        f,
                        "\n- {}: baseline attempts {}, candidate attempts {}",
                        comparison.view,
                        comparison.baseline_attempts,
                        comparison.candidate_attempts
                    )?;
                }
                Ok(())
            }
            Self::Launch { view, message } => {
                write!(f, "could not capture golden view '{view}': {message}")
            }
        }
    }
}

impl std::error::Error for GoldenError {}

fn display_names(names: &[String]) -> String {
    if names.is_empty() {
        "none".into()
    } else {
        names.join(", ")
    }
}

fn solar_sim_manifest_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/solar-sim")
}

fn golden_application_command(
    application: &Path,
    backend: &str,
    view: &str,
    output: &Path,
) -> Command {
    let mut command = Command::new(application);
    command
        // `cargo run -p xtask` exports xtask's manifest directory to this
        // process. Do not let the child inherit it: Bevy uses the variable as
        // its base when resolving solar-sim's `../../assets` source.
        .env("CARGO_MANIFEST_DIR", solar_sim_manifest_dir())
        .env("WGPU_BACKEND", backend)
        .args([
            "--golden-view",
            view,
            "--golden-backend",
            backend,
            "--golden-capture",
        ])
        .arg(output);
    if backend == "metal" {
        command.arg("--reject-software-adapter");
    }
    command
}

/// Launch the already-built application once per canonical view.
pub fn capture_golden_views(
    application: &Path,
    output_root: &Path,
    backend: &str,
) -> Result<GoldenCaptureReport, GoldenError> {
    capture_named_golden_views(application, output_root, backend, &CANONICAL_VIEW_SLUGS)
}

/// Launches the UIO-2 five-scene normal/emphasis orbit review matrix.
pub fn capture_orbit_review_views(
    application: &Path,
    output_root: &Path,
    backend: &str,
) -> Result<GoldenCaptureReport, GoldenError> {
    capture_named_golden_views(application, output_root, backend, &ORBIT_REVIEW_VIEW_SLUGS)
}

/// Launches the UIO-3b size-ratio and dwarf-appearance review matrix.
pub fn capture_scale_review_views(
    application: &Path,
    output_root: &Path,
    backend: &str,
) -> Result<GoldenCaptureReport, GoldenError> {
    capture_named_golden_views(application, output_root, backend, &SCALE_REVIEW_VIEW_SLUGS)
}

fn capture_named_golden_views(
    application: &Path,
    output_root: &Path,
    backend: &str,
    views: &[&str],
) -> Result<GoldenCaptureReport, GoldenError> {
    if backend.is_empty()
        || !backend
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(GoldenError::Configuration(
            "backend tag must contain only lowercase ASCII letters, digits, or '-'".into(),
        ));
    }
    if !application.is_file() {
        return Err(GoldenError::Configuration(format!(
            "application '{}' does not exist",
            application.display()
        )));
    }
    let output = output_root.join(backend);
    fs::create_dir_all(&output).map_err(|error| GoldenError::Read {
        path: output.clone(),
        message: error.to_string(),
    })?;
    let attempts_path = output.join(ATTEMPTS_MANIFEST_FILE);
    if attempts_path.exists() {
        fs::remove_file(&attempts_path).map_err(|error| GoldenError::Read {
            path: attempts_path.clone(),
            message: error.to_string(),
        })?;
    }
    let mut attempts = Vec::with_capacity(views.len());
    for &view in views {
        let path = output.join(format!("{view}.ppm"));
        if path.exists() {
            fs::remove_file(&path).map_err(|error| GoldenError::Read {
                path: path.clone(),
                message: error.to_string(),
            })?;
        }
        let output_result = golden_application_command(application, backend, view, &path)
            .output()
            .map_err(|error| GoldenError::Launch {
                view: view.into(),
                message: error.to_string(),
            })?;
        forward_child_output(view, &output_result.stdout, &output_result.stderr)?;
        if !output_result.status.success() {
            return Err(GoldenError::Launch {
                view: view.into(),
                message: format!("application exited with {}", output_result.status),
            });
        }
        let stdout =
            String::from_utf8(output_result.stdout).map_err(|error| GoldenError::Launch {
                view: view.into(),
                message: format!("application stdout was not UTF-8: {error}"),
            })?;
        attempts.push(
            parse_application_attempts(&stdout, view).map_err(|message| GoldenError::Launch {
                view: view.into(),
                message,
            })?,
        );
    }
    validate_view_set(&output, views)?;
    fs::write(&attempts_path, encode_attempt_manifest(&attempts)).map_err(|error| {
        GoldenError::Read {
            path: attempts_path,
            message: error.to_string(),
        }
    })?;
    Ok(GoldenCaptureReport {
        directory: output,
        attempts,
    })
}

fn forward_child_output(view: &str, stdout: &[u8], stderr: &[u8]) -> Result<(), GoldenError> {
    io::stdout()
        .write_all(stdout)
        .and_then(|()| io::stdout().flush())
        .map_err(|error| GoldenError::Launch {
            view: view.into(),
            message: format!("could not forward application stdout: {error}"),
        })?;
    io::stderr()
        .write_all(stderr)
        .and_then(|()| io::stderr().flush())
        .map_err(|error| GoldenError::Launch {
            view: view.into(),
            message: format!("could not forward application stderr: {error}"),
        })
}

fn parse_application_attempts(
    stdout: &str,
    expected_view: &str,
) -> Result<GoldenAttemptCount, String> {
    let mut parsed = None;
    for line in stdout
        .lines()
        .filter(|line| line.starts_with("golden-attempts"))
    {
        let count = parse_attempt_line(line)?;
        if count.view != expected_view {
            return Err(format!(
                "attempt report named view '{}' instead of '{expected_view}'",
                count.view
            ));
        }
        if parsed.replace(count).is_some() {
            return Err(format!(
                "application emitted more than one attempt report for '{expected_view}'"
            ));
        }
    }
    parsed.ok_or_else(|| {
        format!("application emitted no golden-attempts report for '{expected_view}'")
    })
}

fn parse_attempt_line(line: &str) -> Result<GoldenAttemptCount, String> {
    let mut fields = line.split_ascii_whitespace();
    if fields.next() != Some("golden-attempts") {
        return Err("attempt line must start with 'golden-attempts'".into());
    }
    let view = fields
        .next()
        .and_then(|field| field.strip_prefix("view="))
        .filter(|view| !view.is_empty())
        .ok_or_else(|| "attempt line needs a non-empty view=<slug> field".to_string())?;
    let attempts = fields
        .next()
        .and_then(|field| field.strip_prefix("attempts="))
        .ok_or_else(|| "attempt line needs an attempts=<n> field".to_string())?
        .parse::<u8>()
        .map_err(|error| format!("attempt count is not an unsigned byte: {error}"))?;
    if attempts == 0 {
        return Err("attempt count must be greater than zero".into());
    }
    if fields.next().is_some() {
        return Err("attempt line has unexpected trailing fields".into());
    }
    Ok(GoldenAttemptCount {
        view: view.into(),
        attempts,
    })
}

fn encode_attempt_manifest(attempts: &[GoldenAttemptCount]) -> String {
    let mut manifest = String::new();
    for count in attempts {
        manifest.push_str(&format!(
            "golden-attempts view={} attempts={}\n",
            count.view, count.attempts
        ));
    }
    manifest
}

fn read_attempt_manifest(
    directory: &Path,
    views: &[&str],
) -> Result<Vec<GoldenAttemptCount>, GoldenError> {
    let path = directory.join(ATTEMPTS_MANIFEST_FILE);
    let manifest = fs::read_to_string(&path).map_err(|error| GoldenError::Read {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let mut attempts = Vec::new();
    for line in manifest.lines().filter(|line| !line.is_empty()) {
        attempts.push(
            parse_attempt_line(line).map_err(|message| GoldenError::Attempts {
                path: path.clone(),
                message,
            })?,
        );
    }
    let missing = views
        .iter()
        .filter(|view| !attempts.iter().any(|count| count.view == **view))
        .copied()
        .collect::<Vec<_>>();
    let unexpected = attempts
        .iter()
        .filter(|count| !views.contains(&count.view.as_str()))
        .map(|count| count.view.as_str())
        .collect::<Vec<_>>();
    let duplicate = attempts.iter().find_map(|count| {
        (attempts
            .iter()
            .filter(|other| other.view == count.view)
            .count()
            > 1)
        .then_some(count.view.as_str())
    });
    if !missing.is_empty() || !unexpected.is_empty() || duplicate.is_some() {
        return Err(GoldenError::Attempts {
            path,
            message: format!(
                "wrong view set (missing: {}; unexpected: {}; duplicate: {})",
                missing.join(", "),
                unexpected.join(", "),
                duplicate.unwrap_or("none")
            ),
        });
    }
    Ok(attempts)
}

fn attempts_for_view(
    attempts: &[GoldenAttemptCount],
    view: &str,
    directory: &Path,
) -> Result<u8, GoldenError> {
    attempts
        .iter()
        .find(|count| count.view == view)
        .map(|count| count.attempts)
        .ok_or_else(|| GoldenError::Attempts {
            path: directory.join(ATTEMPTS_MANIFEST_FILE),
            message: format!("missing view '{view}'"),
        })
}

pub fn compare_golden_directories(
    baseline: &Path,
    candidate: &Path,
    threshold: PerceptualThreshold,
    allow_retries: bool,
) -> Result<Vec<GoldenComparison>, GoldenError> {
    compare_named_golden_directories(
        baseline,
        candidate,
        threshold,
        allow_retries,
        &CANONICAL_VIEW_SLUGS,
    )
}

pub fn compare_orbit_review_directories(
    baseline: &Path,
    candidate: &Path,
    threshold: PerceptualThreshold,
    allow_retries: bool,
) -> Result<Vec<GoldenComparison>, GoldenError> {
    compare_named_golden_directories(
        baseline,
        candidate,
        threshold,
        allow_retries,
        &ORBIT_REVIEW_VIEW_SLUGS,
    )
}

pub fn compare_scale_review_directories(
    baseline: &Path,
    candidate: &Path,
    threshold: PerceptualThreshold,
    allow_retries: bool,
) -> Result<Vec<GoldenComparison>, GoldenError> {
    compare_named_golden_directories(
        baseline,
        candidate,
        threshold,
        allow_retries,
        &SCALE_REVIEW_VIEW_SLUGS,
    )
}

fn compare_named_golden_directories(
    baseline: &Path,
    candidate: &Path,
    threshold: PerceptualThreshold,
    allow_retries: bool,
    views: &[&str],
) -> Result<Vec<GoldenComparison>, GoldenError> {
    let threshold = threshold.validate()?;
    validate_view_set(baseline, views)?;
    validate_view_set(candidate, views)?;
    let baseline_attempts = read_attempt_manifest(baseline, views)?;
    let candidate_attempts = read_attempt_manifest(candidate, views)?;
    let mut comparisons = Vec::with_capacity(views.len());
    for &view in views {
        let baseline_image = read_ppm(&baseline.join(format!("{view}.ppm")), view)?;
        let candidate_image = read_ppm(&candidate.join(format!("{view}.ppm")), view)?;
        let baseline_attempts = attempts_for_view(&baseline_attempts, view, baseline)?;
        let candidate_attempts = attempts_for_view(&candidate_attempts, view, candidate)?;
        comparisons.push(compare_images(
            view,
            &baseline_image,
            &candidate_image,
            threshold,
            baseline_attempts,
            candidate_attempts,
        )?);
    }
    if comparisons.iter().any(|comparison| !comparison.passed) {
        return Err(GoldenError::ThresholdExceeded(comparisons));
    }
    if !allow_retries
        && comparisons
            .iter()
            .any(|comparison| comparison.baseline_attempts > 1 || comparison.candidate_attempts > 1)
    {
        return Err(GoldenError::RetriesDetected(comparisons));
    }
    Ok(comparisons)
}

fn validate_view_set(directory: &Path, views: &[&str]) -> Result<(), GoldenError> {
    let entries = fs::read_dir(directory).map_err(|error| GoldenError::Read {
        path: directory.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut actual = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| GoldenError::Read {
            path: directory.to_path_buf(),
            message: error.to_string(),
        })?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("ppm") {
            if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
                actual.push(stem.to_string());
            }
        }
    }
    actual.sort();
    let mut expected: Vec<_> = views.iter().map(|slug| (*slug).to_string()).collect();
    expected.sort();
    let missing = expected
        .iter()
        .filter(|name| !actual.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    let unexpected = actual
        .iter()
        .filter(|name| !expected.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() && unexpected.is_empty() {
        Ok(())
    } else {
        Err(GoldenError::ViewSet {
            directory: directory.to_path_buf(),
            missing,
            unexpected,
        })
    }
}

fn read_ppm(path: &Path, view: &str) -> Result<RasterImage, GoldenError> {
    let bytes = fs::read(path).map_err(|error| GoldenError::Read {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    parse_binary_ppm(&bytes).map_err(|error| GoldenError::Image {
        view: view.into(),
        message: match error {
            TexturePipelineError::InvalidPpm(message) => message,
            other => other.to_string(),
        },
    })
}

fn compare_images(
    view: &str,
    baseline: &RasterImage,
    candidate: &RasterImage,
    threshold: PerceptualThreshold,
    baseline_attempts: u8,
    candidate_attempts: u8,
) -> Result<GoldenComparison, GoldenError> {
    if baseline.width != candidate.width
        || baseline.height != candidate.height
        || baseline.channels != 3
        || candidate.channels != 3
    {
        return Err(GoldenError::Image {
            view: view.into(),
            message: format!(
                "baseline is {}x{}x{} but candidate is {}x{}x{}",
                baseline.width,
                baseline.height,
                baseline.channels,
                candidate.width,
                candidate.height,
                candidate.channels
            ),
        });
    }
    let mut deltas = Vec::with_capacity(baseline.pixels.len() / 3);
    for (left, right) in baseline
        .pixels
        .chunks_exact(3)
        .zip(candidate.pixels.chunks_exact(3))
    {
        let left_lab = srgb_to_lab([left[0], left[1], left[2]]);
        let right_lab = srgb_to_lab([right[0], right[1], right[2]]);
        deltas.push(
            ((left_lab[0] - right_lab[0]).powi(2)
                + (left_lab[1] - right_lab[1]).powi(2)
                + (left_lab[2] - right_lab[2]).powi(2))
            .sqrt(),
        );
    }
    if deltas.is_empty() {
        return Err(GoldenError::Image {
            view: view.into(),
            message: "image contains no pixels".into(),
        });
    }
    let mean = deltas.iter().sum::<f64>() / deltas.len() as f64;
    deltas.sort_by(f64::total_cmp);
    let p99_index = ((deltas.len() as f64 * 0.99).ceil() as usize)
        .saturating_sub(1)
        .min(deltas.len() - 1);
    let p99 = deltas[p99_index];
    Ok(GoldenComparison {
        view: view.into(),
        width: baseline.width,
        height: baseline.height,
        mean_delta_e: mean,
        p99_delta_e: p99,
        baseline_attempts,
        candidate_attempts,
        passed: mean <= threshold.max_mean_delta_e && p99 <= threshold.max_p99_delta_e,
    })
}

fn srgb_to_lab(rgb: [u8; 3]) -> [f64; 3] {
    fn linear(channel: u8) -> f64 {
        let value = f64::from(channel) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    let r = linear(rgb[0]);
    let g = linear(rgb[1]);
    let b = linear(rgb[2]);
    // sRGB/BT.709 to CIE XYZ (D65), then CIE L*a*b*.
    let x = (0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b) / 0.950_47;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175 * b;
    let z = (0.019_333_9 * r + 0.119_192 * g + 0.950_304_1 * b) / 1.088_83;
    fn lab_curve(value: f64) -> f64 {
        const EPSILON: f64 = 216.0 / 24_389.0;
        const KAPPA: f64 = 24_389.0 / 27.0;
        if value > EPSILON {
            value.cbrt()
        } else {
            (KAPPA * value + 16.0) / 116.0
        }
    }
    let fx = lab_curve(x);
    let fy = lab_curve(y);
    let fz = lab_curve(z);
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let index = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "solar-sim-golden-{label}-{}-{index}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_view_set(directory: &Path, color: [u8; 3]) {
        for view in CANONICAL_VIEW_SLUGS {
            let mut ppm = b"P6\n4 4\n255\n".to_vec();
            for _ in 0..16 {
                ppm.extend_from_slice(&color);
            }
            fs::write(directory.join(format!("{view}.ppm")), ppm).unwrap();
        }
        write_attempt_manifest(directory, None);
    }

    fn write_attempt_manifest(directory: &Path, retry_view: Option<&str>) {
        let attempts = CANONICAL_VIEW_SLUGS
            .iter()
            .map(|view| GoldenAttemptCount {
                view: (*view).into(),
                attempts: if retry_view == Some(*view) { 2 } else { 1 },
            })
            .collect::<Vec<_>>();
        fs::write(
            directory.join(ATTEMPTS_MANIFEST_FILE),
            encode_attempt_manifest(&attempts),
        )
        .unwrap();
    }

    #[test]
    fn golden_child_anchors_bevy_assets_to_the_solar_sim_manifest() {
        let command = golden_application_command(
            Path::new("solar-sim-test"),
            "metal",
            "full-system",
            Path::new("capture.ppm"),
        );
        let child_manifest = command
            .get_envs()
            .find_map(|(key, value)| {
                (key == "CARGO_MANIFEST_DIR").then(|| value.unwrap().to_owned())
            })
            .expect("golden child must override Cargo's xtask manifest directory");
        let child_asset_root = Path::new(&child_manifest)
            .join("../../assets")
            .canonicalize()
            .unwrap();
        let workspace_asset_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../assets")
            .canonicalize()
            .unwrap();
        assert_eq!(child_asset_root, workspace_asset_root);
    }

    #[test]
    fn metal_golden_children_reject_software_adapters_but_dx12_children_do_not() {
        for (backend, expected) in [("metal", true), ("dx12", false)] {
            let command = golden_application_command(
                Path::new("solar-sim-test"),
                backend,
                "full-system",
                Path::new("capture.ppm"),
            );
            let has_rejection = command
                .get_args()
                .any(|argument| argument == "--reject-software-adapter");
            assert_eq!(has_rejection, expected);
        }
    }

    #[test]
    fn golden_harness_requires_exactly_the_six_canonical_views() {
        let directory = TestDir::new("set");
        write_view_set(&directory.0, [10, 20, 30]);
        assert!(validate_view_set(&directory.0, &CANONICAL_VIEW_SLUGS).is_ok());
        fs::remove_file(directory.0.join("sun-bloom.ppm")).unwrap();
        fs::write(directory.0.join("seventh.ppm"), b"P6\n1 1\n255\n\0\0\0").unwrap();
        let error = validate_view_set(&directory.0, &CANONICAL_VIEW_SLUGS).unwrap_err();
        let GoldenError::ViewSet {
            missing,
            unexpected,
            ..
        } = error
        else {
            panic!("wrong error: {error:?}");
        };
        assert_eq!(missing, vec!["sun-bloom"]);
        assert_eq!(unexpected, vec!["seventh"]);
    }

    #[test]
    fn application_attempt_parser_accepts_one_exact_record_and_rejects_corruption() {
        let stdout = "renderer ready\ngolden-attempts view=earth-texture attempts=2\ndone\n";
        assert_eq!(
            parse_application_attempts(stdout, "earth-texture").unwrap(),
            GoldenAttemptCount {
                view: "earth-texture".into(),
                attempts: 2,
            }
        );
        for corrupt in [
            "renderer ready\n",
            "golden-attempts view=earth-texture attempts=0\n",
            "golden-attempts view=saturn-rings attempts=1\n",
            "golden-attempts view=earth-texture attempts=1 extra=true\n",
            "golden-attempts view=earth-texture attempts=1\ngolden-attempts view=earth-texture attempts=2\n",
        ] {
            assert!(parse_application_attempts(corrupt, "earth-texture").is_err());
        }
    }

    #[test]
    fn perceptual_threshold_accepts_small_backend_noise_and_rejects_scene_drift() {
        let baseline = TestDir::new("baseline");
        let candidate = TestDir::new("candidate");
        write_view_set(&baseline.0, [80, 120, 160]);
        write_view_set(&candidate.0, [81, 120, 160]);
        let comparisons = compare_golden_directories(
            &baseline.0,
            &candidate.0,
            PerceptualThreshold::default(),
            false,
        )
        .unwrap();
        assert_eq!(comparisons.len(), 6);
        assert!(comparisons.iter().all(|comparison| comparison.passed));

        write_view_set(&candidate.0, [220, 30, 20]);
        let error = compare_golden_directories(
            &baseline.0,
            &candidate.0,
            PerceptualThreshold::default(),
            false,
        )
        .unwrap_err();
        let GoldenError::ThresholdExceeded(comparisons) = error else {
            panic!("wrong error: {error:?}");
        };
        assert_eq!(comparisons.len(), 6);
        assert!(comparisons.iter().all(|comparison| !comparison.passed));
    }

    #[test]
    fn comparison_rejects_retries_by_default_and_allows_explicit_diagnostics() {
        let baseline = TestDir::new("attempt-baseline");
        let candidate = TestDir::new("attempt-candidate");
        write_view_set(&baseline.0, [80, 120, 160]);
        write_view_set(&candidate.0, [80, 120, 160]);
        write_attempt_manifest(&candidate.0, Some("earth-texture"));

        let error = compare_golden_directories(
            &baseline.0,
            &candidate.0,
            PerceptualThreshold::default(),
            false,
        )
        .unwrap_err();
        let GoldenError::RetriesDetected(comparisons) = error else {
            panic!("wrong error: {error:?}");
        };
        let earth = comparisons
            .iter()
            .find(|comparison| comparison.view == "earth-texture")
            .unwrap();
        assert_eq!((earth.baseline_attempts, earth.candidate_attempts), (1, 2));

        assert!(compare_golden_directories(
            &baseline.0,
            &candidate.0,
            PerceptualThreshold::default(),
            true,
        )
        .is_ok());
    }

    #[test]
    fn corrupt_golden_pixels_are_rejected() {
        let baseline = TestDir::new("corrupt-baseline");
        let candidate = TestDir::new("corrupt-candidate");
        write_view_set(&baseline.0, [10, 20, 30]);
        write_view_set(&candidate.0, [10, 20, 30]);
        fs::write(candidate.0.join("earth-texture.ppm"), b"not ppm").unwrap();
        assert!(matches!(
            compare_golden_directories(
                &baseline.0,
                &candidate.0,
                PerceptualThreshold::default(),
                false,
            ),
            Err(GoldenError::Image { .. })
        ));
    }
}
