//! UIP-1 — strict frame-stats ingestion and WP17 evidence-table formatting.

use serde::Deserialize;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const FRAME_STATS_SCHEMA: &str = "solar-sim-frame-stats-v2";
const VIEW_ORDER: [&str; 7] = [
    "full-system",
    "inner-orbits",
    "earth-texture",
    "jupiter-system",
    "saturn-rings",
    "sun-bloom",
    "interactive",
];
const QUALITY_ORDER: [&str; 4] = ["low", "medium", "high", "ultra"];

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FrameStatsSummary {
    pub schema: String,
    pub view: String,
    pub duration_s: f64,
    pub frames: usize,
    pub min_ms: f64,
    pub mean_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub fps: f64,
    pub width_px: u32,
    pub height_px: u32,
    pub msaa_requested: String,
    pub msaa_effective: String,
    pub quality: String,
    pub vsync: bool,
    pub frame_cap: String,
    pub adapter_name: String,
    pub adapter_type: String,
    pub backend: String,
    pub adapter_driver: String,
    pub adapter_driver_info: String,
    pub adapter_vendor: u32,
    pub adapter_device: u32,
}

#[derive(Debug)]
pub enum PerfReportError {
    Read { path: PathBuf, message: String },
    Parse { path: PathBuf, message: String },
    Invalid { path: PathBuf, message: String },
}

impl fmt::Display for PerfReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, message } => {
                write!(f, "could not read '{}': {message}", path.display())
            }
            Self::Parse { path, message } => {
                write!(
                    f,
                    "invalid frame-stats JSON in '{}': {message}",
                    path.display()
                )
            }
            Self::Invalid { path, message } => {
                write!(
                    f,
                    "invalid frame-stats values in '{}': {message}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for PerfReportError {}

pub fn read_summaries(paths: &[PathBuf]) -> Result<Vec<FrameStatsSummary>, PerfReportError> {
    let mut summaries = paths
        .iter()
        .map(|path| read_summary(path))
        .collect::<Result<Vec<_>, _>>()?;
    summaries.sort_by_key(|summary| {
        (
            order_of(&VIEW_ORDER, &summary.view),
            order_of(&QUALITY_ORDER, &summary.quality),
            summary.view.clone(),
            summary.quality.clone(),
        )
    });
    Ok(summaries)
}

fn read_summary(path: &Path) -> Result<FrameStatsSummary, PerfReportError> {
    let text = fs::read_to_string(path).map_err(|error| PerfReportError::Read {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let line = lines.next().ok_or_else(|| PerfReportError::Parse {
        path: path.to_path_buf(),
        message: "expected exactly one non-empty summary line".into(),
    })?;
    if lines.next().is_some() {
        return Err(PerfReportError::Parse {
            path: path.to_path_buf(),
            message: "expected exactly one non-empty summary line".into(),
        });
    }
    parse_summary_line(path, line)
}

fn parse_summary_line(path: &Path, line: &str) -> Result<FrameStatsSummary, PerfReportError> {
    let summary = serde_json::from_str::<FrameStatsSummary>(line).map_err(|error| {
        PerfReportError::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    validate_summary(path, &summary)?;
    Ok(summary)
}

fn validate_summary(path: &Path, summary: &FrameStatsSummary) -> Result<(), PerfReportError> {
    let invalid = |message: &str| PerfReportError::Invalid {
        path: path.to_path_buf(),
        message: message.into(),
    };
    if summary.schema != FRAME_STATS_SCHEMA {
        return Err(invalid("unsupported schema"));
    }
    if summary.view.is_empty() || summary.adapter_name.is_empty() || summary.backend.is_empty() {
        return Err(invalid(
            "view and adapter identity fields must be non-empty",
        ));
    }
    if !QUALITY_ORDER.contains(&summary.quality.as_str()) {
        return Err(invalid("quality must be low, medium, high, or ultra"));
    }
    if summary.frames == 0 || summary.width_px == 0 || summary.height_px == 0 {
        return Err(invalid("frame count and resolution must be non-zero"));
    }
    let requested_msaa = msaa_count(&summary.msaa_requested)
        .ok_or_else(|| invalid("requested MSAA must be off, 2x, 4x, or 8x"))?;
    let effective_msaa = msaa_count(&summary.msaa_effective)
        .ok_or_else(|| invalid("effective MSAA must be off, 2x, 4x, or 8x"))?;
    if effective_msaa > requested_msaa {
        return Err(invalid("effective MSAA cannot exceed its preset request"));
    }
    for (name, value) in [
        ("duration_s", summary.duration_s),
        ("min_ms", summary.min_ms),
        ("mean_ms", summary.mean_ms),
        ("p95_ms", summary.p95_ms),
        ("p99_ms", summary.p99_ms),
        ("fps", summary.fps),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(invalid(&format!("{name} must be positive and finite")));
        }
    }
    if summary.min_ms > summary.mean_ms
        || summary.min_ms > summary.p95_ms
        || summary.p95_ms > summary.p99_ms
    {
        return Err(invalid(
            "frame-time statistics are not monotonically ordered",
        ));
    }
    Ok(())
}

fn msaa_count(value: &str) -> Option<u8> {
    match value {
        "off" => Some(1),
        "2x" => Some(2),
        "4x" => Some(4),
        "8x" => Some(8),
        _ => None,
    }
}

fn order_of(order: &[&str], value: &str) -> usize {
    order
        .iter()
        .position(|candidate| *candidate == value)
        .unwrap_or(order.len())
}

pub fn format_wp17_table(summaries: &[FrameStatsSummary]) -> String {
    let mut output = String::from(
        "| View | Quality | Resolution | MSAA requested | MSAA effective | VSync | Frame cap | Frames | Min ms | Mean ms | P95 ms | P99 ms | FPS | Adapter |\n\
         |---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|\n",
    );
    for summary in summaries {
        let adapter = markdown_cell(&format!(
            "{} ({}, {})",
            summary.adapter_name, summary.adapter_type, summary.backend
        ));
        output.push_str(&format!(
            "| {} | {} | {}×{} | {} | {} | {} | {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.1} | {} |\n",
            markdown_cell(&summary.view),
            markdown_cell(&summary.quality),
            summary.width_px,
            summary.height_px,
            markdown_cell(&summary.msaa_requested),
            markdown_cell(&summary.msaa_effective),
            if summary.vsync { "on" } else { "off" },
            markdown_cell(&summary.frame_cap),
            summary.frames,
            summary.min_ms,
            summary.mean_ms,
            summary.p95_ms,
            summary.p99_ms,
            summary.fps,
            adapter,
        ));
    }
    output
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(view: &str, quality: &str) -> FrameStatsSummary {
        FrameStatsSummary {
            schema: FRAME_STATS_SCHEMA.into(),
            view: view.into(),
            duration_s: 5.0,
            frames: 600,
            min_ms: 7.0,
            mean_ms: 8.0,
            p95_ms: 9.0,
            p99_ms: 10.0,
            fps: 125.0,
            width_px: 3456,
            height_px: 2160,
            msaa_requested: "4x".into(),
            msaa_effective: "4x".into(),
            quality: quality.into(),
            vsync: true,
            frame_cap: "120".into(),
            adapter_name: "Apple M2 Pro".into(),
            adapter_type: "IntegratedGpu".into(),
            backend: "metal".into(),
            adapter_driver: String::new(),
            adapter_driver_info: String::new(),
            adapter_vendor: 0,
            adapter_device: 0,
        }
    }

    #[test]
    fn wp17_table_format_is_golden_tested() {
        assert_eq!(
            format_wp17_table(&[summary("full-system", "high")]),
            concat!(
                "| View | Quality | Resolution | MSAA requested | MSAA effective | VSync | Frame cap | Frames | Min ms | Mean ms | P95 ms | P99 ms | FPS | Adapter |\n",
                "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|\n",
                "| full-system | high | 3456×2160 | 4x | 4x | on | 120 | 600 | 7.000 | 8.000 | 9.000 | 10.000 | 125.0 | Apple M2 Pro (IntegratedGpu, metal) |\n",
            )
        );
    }

    #[test]
    fn reports_sort_by_canonical_view_then_quality() {
        let mut summaries = [
            summary("sun-bloom", "ultra"),
            summary("full-system", "high"),
            summary("full-system", "low"),
        ];
        summaries.sort_by_key(|summary| {
            (
                order_of(&VIEW_ORDER, &summary.view),
                order_of(&QUALITY_ORDER, &summary.quality),
            )
        });
        assert_eq!(
            summaries
                .iter()
                .map(|summary| (summary.view.as_str(), summary.quality.as_str()))
                .collect::<Vec<_>>(),
            [
                ("full-system", "low"),
                ("full-system", "high"),
                ("sun-bloom", "ultra"),
            ]
        );
    }

    #[test]
    fn validation_rejects_corrupt_schema_and_statistics() {
        let path = Path::new("bad.json");
        let mut invalid = summary("full-system", "high");
        invalid.schema = "future".into();
        assert!(validate_summary(path, &invalid).is_err());
        invalid.schema = FRAME_STATS_SCHEMA.into();
        invalid.p95_ms = 10.5;
        invalid.p99_ms = 10.0;
        assert!(validate_summary(path, &invalid).is_err());
        invalid.p95_ms = 9.0;
        invalid.msaa_requested = "4x".into();
        invalid.msaa_effective = "8x".into();
        assert!(validate_summary(path, &invalid).is_err());
    }

    #[test]
    fn parser_rejects_unknown_fields_and_missing_adapter_identity() {
        let path = Path::new("bad.json");
        let valid = serde_json::to_string(&serde_json::json!({
            "schema": FRAME_STATS_SCHEMA,
            "view": "full-system",
            "duration_s": 1.0,
            "frames": 120,
            "min_ms": 7.0,
            "mean_ms": 8.0,
            "p95_ms": 9.0,
            "p99_ms": 10.0,
            "fps": 125.0,
            "width_px": 1920,
            "height_px": 1080,
            "msaa_requested": "4x",
            "msaa_effective": "4x",
            "quality": "high",
            "vsync": true,
            "frame_cap": "120",
            "adapter_name": "GPU",
            "adapter_type": "DiscreteGpu",
            "backend": "vulkan",
            "adapter_driver": "driver",
            "adapter_driver_info": "info",
            "adapter_vendor": 1,
            "adapter_device": 2
        }))
        .unwrap();
        assert!(parse_summary_line(path, &valid).is_ok());

        let unknown = valid.replacen("{", "{\"unexpected\":true,", 1);
        assert!(parse_summary_line(path, &unknown).is_err());
        let missing = valid.replace("\"adapter_name\":\"GPU\"", "\"adapter_name\":\"\"");
        assert!(parse_summary_line(path, &missing).is_err());
    }

    #[test]
    fn markdown_cells_cannot_break_the_evidence_table() {
        assert_eq!(markdown_cell("GPU | Metal\nDriver"), "GPU \\| Metal Driver");
    }
}
