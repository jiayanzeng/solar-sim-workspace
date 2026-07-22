//! CLI:
//!   cargo run -p xtask -- gen-catalog --dry-run
//!   cargo run -p xtask -- gen-catalog --fixtures xtask/fixtures --allow-partial --out assets/catalog.sample.ron
//!   cargo run -p xtask --features online -- gen-catalog --online --capture xtask/fixtures/captured-YYYY-MM --out assets/catalog.ron
//!   cargo run -p xtask -- bake-starfield --source bsc5p.vot --out assets/starfield.bsc
//!   cargo run -p xtask -- prepare-steam-dev --app target/release/solar-sim

use anyhow::{bail, Result};
use std::path::PathBuf;
use xtask::{
    emit, fetch, golden, perf, plan, starfield, steam, texture, GenOptions, DEFAULT_EPOCH_JD_TDB,
};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("gen-catalog") => gen_catalog(&args[1..]),
        Some("bake-starfield") => bake_starfield(&args[1..]),
        Some("convert-texture") => convert_texture(&args[1..]),
        Some("check-texture-metadata") => check_texture_metadata(&args[1..]),
        Some("capture-goldens") => capture_goldens(&args[1..]),
        Some("compare-goldens") => compare_goldens(&args[1..]),
        Some("perf-report") => perf_report(&args[1..]),
        Some("prepare-steam-dev") => prepare_steam_dev(&args[1..]),
        Some("steam-release-preflight") => steam_release_preflight(&args[1..]),
        _ => {
            eprintln!("usage:\n  xtask gen-catalog [--out PATH] [--epoch-jd F] [--dry-run] [--fixtures DIR [--allow-partial]] [--online [--capture DIR]]\n  xtask bake-starfield --source PATH --out PATH [--limit N]\n  xtask convert-texture --source PATH.ppm --out PATH.ktx2 [--alpha-from-luminance]\n  xtask check-texture-metadata [--dir assets/textures]\n  xtask capture-goldens --app PATH --out DIR --backend TAG\n  xtask compare-goldens --baseline DIR --candidate DIR [--max-mean F] [--max-p99 F] [--allow-retries]\n  xtask perf-report STATS.json [STATS.json ...]\n  xtask prepare-steam-dev --app PATH\n  xtask steam-release-preflight --action package|depot");
            std::process::exit(2);
        }
    }
}

fn perf_report(args: &[String]) -> Result<()> {
    if args.is_empty() {
        bail!("perf-report requires at least one frame-stats summary path");
    }
    if let Some(flag) = args.iter().find(|argument| argument.starts_with('-')) {
        bail!("unknown perf-report flag: {flag}");
    }
    let paths = args.iter().map(PathBuf::from).collect::<Vec<_>>();
    let summaries = perf::read_summaries(&paths)?;
    print!("{}", perf::format_wp17_table(&summaries));
    Ok(())
}

fn prepare_steam_dev(args: &[String]) -> Result<()> {
    let mut application: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--app" => {
                i += 1;
                application = Some(PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--app needs a path"))?,
                ));
            }
            other => bail!("unknown flag: {other}"),
        }
        i += 1;
    }
    let output = steam::prepare_development_application(
        &application.ok_or_else(|| anyhow::anyhow!("--app is required"))?,
    )?;
    println!(
        "generated {} from STEAM_APP_ID={} (development only)",
        output.display(),
        steam::STEAM_APP_ID
    );
    Ok(())
}

fn steam_release_preflight(args: &[String]) -> Result<()> {
    let mut action: Option<steam::ReleaseAction> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--action" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| anyhow::anyhow!("--action needs package or depot"))?;
                action = Some(steam::ReleaseAction::parse(value).ok_or_else(|| {
                    anyhow::anyhow!(
                        "unsupported release action '{value}'; expected package or depot"
                    )
                })?);
            }
            other => bail!("unknown flag: {other}"),
        }
        i += 1;
    }
    let action = action.ok_or_else(|| anyhow::anyhow!("--action is required"))?;
    steam::require_release_app_id(action)?;
    println!(
        "Steam {action} App-ID preflight passed for {}",
        steam::STEAM_APP_ID
    );
    Ok(())
}

fn convert_texture(args: &[String]) -> Result<()> {
    let mut source: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut alpha_mode = texture::AlphaMode::Opaque;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--source" => {
                i += 1;
                source = Some(PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--source needs a path"))?,
                ));
            }
            "--out" => {
                i += 1;
                out = Some(PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--out needs a path"))?,
                ));
            }
            "--alpha-from-luminance" => alpha_mode = texture::AlphaMode::FromLuminance,
            other => bail!("unknown flag: {other}"),
        }
        i += 1;
    }
    let source = source.ok_or_else(|| anyhow::anyhow!("--source is required"))?;
    let out = out.ok_or_else(|| anyhow::anyhow!("--out is required"))?;
    let image = texture::convert_texture_file(&source, &out, alpha_mode)?;
    println!(
        "wrote {} ({}x{}, {} channels)",
        out.display(),
        image.width,
        image.height,
        image.channels
    );
    Ok(())
}

fn check_texture_metadata(args: &[String]) -> Result<()> {
    let mut directory = PathBuf::from("assets/textures");
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" => {
                i += 1;
                directory = PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--dir needs a path"))?,
                );
            }
            other => bail!("unknown flag: {other}"),
        }
        i += 1;
    }
    let count = texture::audit_texture_directory(&directory)?;
    println!(
        "texture metadata audit passed: {count} KTX2 assets in {}",
        directory.display()
    );
    Ok(())
}

fn capture_goldens(args: &[String]) -> Result<()> {
    let mut application: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut backend: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--app" => {
                i += 1;
                application = Some(PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--app needs a path"))?,
                ));
            }
            "--out" => {
                i += 1;
                output = Some(PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--out needs a path"))?,
                ));
            }
            "--backend" => {
                i += 1;
                backend = Some(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--backend needs a tag"))?
                        .clone(),
                );
            }
            other => bail!("unknown flag: {other}"),
        }
        i += 1;
    }
    let report = golden::capture_golden_views(
        &application.ok_or_else(|| anyhow::anyhow!("--app is required"))?,
        &output.ok_or_else(|| anyhow::anyhow!("--out is required"))?,
        &backend.ok_or_else(|| anyhow::anyhow!("--backend is required"))?,
    )?;
    println!(
        "golden attempts: {}",
        report
            .attempts
            .iter()
            .map(|count| format!("{}={}", count.view, count.attempts))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "captured {} canonical views in {}",
        golden::CANONICAL_VIEW_SLUGS.len(),
        report.directory.display()
    );
    Ok(())
}

fn compare_goldens(args: &[String]) -> Result<()> {
    let mut baseline: Option<PathBuf> = None;
    let mut candidate: Option<PathBuf> = None;
    let mut threshold = golden::PerceptualThreshold::default();
    let mut allow_retries = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--baseline" => {
                i += 1;
                baseline = Some(PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--baseline needs a path"))?,
                ));
            }
            "--candidate" => {
                i += 1;
                candidate =
                    Some(PathBuf::from(args.get(i).ok_or_else(|| {
                        anyhow::anyhow!("--candidate needs a path")
                    })?));
            }
            "--max-mean" => {
                i += 1;
                threshold.max_mean_delta_e = args
                    .get(i)
                    .ok_or_else(|| anyhow::anyhow!("--max-mean needs a value"))?
                    .parse()?;
            }
            "--max-p99" => {
                i += 1;
                threshold.max_p99_delta_e = args
                    .get(i)
                    .ok_or_else(|| anyhow::anyhow!("--max-p99 needs a value"))?
                    .parse()?;
            }
            "--allow-retries" => allow_retries = true,
            other => bail!("unknown flag: {other}"),
        }
        i += 1;
    }
    let baseline = baseline.ok_or_else(|| anyhow::anyhow!("--baseline is required"))?;
    let candidate = candidate.ok_or_else(|| anyhow::anyhow!("--candidate is required"))?;
    match golden::compare_golden_directories(&baseline, &candidate, threshold, allow_retries) {
        Ok(comparisons) => {
            print_golden_comparisons(&comparisons);
            Ok(())
        }
        Err(golden::GoldenError::ThresholdExceeded(comparisons)) => {
            print_golden_comparisons(&comparisons);
            bail!("one or more golden views exceeded the perceptual threshold")
        }
        Err(golden::GoldenError::RetriesDetected(comparisons)) => {
            print_golden_comparisons(&comparisons);
            bail!("one or more golden views required a retry; use --allow-retries only for an explicitly reviewed diagnostic run")
        }
        Err(error) => Err(error.into()),
    }
}

fn print_golden_comparisons(comparisons: &[golden::GoldenComparison]) {
    for comparison in comparisons {
        println!(
            "{:<16} {}x{} mean ΔE={:.4} p99 ΔE={:.4} attempts={}/{} {}",
            comparison.view,
            comparison.width,
            comparison.height,
            comparison.mean_delta_e,
            comparison.p99_delta_e,
            comparison.baseline_attempts,
            comparison.candidate_attempts,
            if comparison.passed { "PASS" } else { "FAIL" }
        );
    }
}

fn bake_starfield(args: &[String]) -> Result<()> {
    let mut source: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut limit = starfield::DEFAULT_STAR_LIMIT;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--source" => {
                i += 1;
                source = Some(PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--source needs a path"))?,
                ));
            }
            "--out" => {
                i += 1;
                out = Some(PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--out needs a path"))?,
                ));
            }
            "--limit" => {
                i += 1;
                limit = args
                    .get(i)
                    .ok_or_else(|| anyhow::anyhow!("--limit needs a value"))?
                    .parse()?;
            }
            other => bail!("unknown flag: {other}"),
        }
        i += 1;
    }
    let source = source.ok_or_else(|| anyhow::anyhow!("--source is required"))?;
    let out = out.ok_or_else(|| anyhow::anyhow!("--out is required"))?;
    if limit == 0 {
        bail!("--limit must be greater than zero");
    }
    let count = starfield::bake_catalog_file(&source, &out, limit)?;
    println!(
        "wrote {} ({count} brightest BSC stars from {})",
        out.display(),
        source.display()
    );
    Ok(())
}

fn gen_catalog(args: &[String]) -> Result<()> {
    let mut out = PathBuf::from("assets/catalog.ron");
    let mut epoch = DEFAULT_EPOCH_JD_TDB;
    let mut fixtures: Option<PathBuf> = None;
    let mut online = false;
    let mut dry_run = false;
    let mut allow_partial = false;
    let mut capture: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out = PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--out needs a path"))?,
                );
            }
            "--epoch-jd" => {
                i += 1;
                epoch = args
                    .get(i)
                    .ok_or_else(|| anyhow::anyhow!("--epoch-jd needs a value"))?
                    .parse()?;
            }
            "--fixtures" => {
                i += 1;
                fixtures = Some(PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--fixtures needs a dir"))?,
                ));
            }
            "--online" => online = true,
            "--capture" => {
                i += 1;
                capture = Some(PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--capture needs a dir"))?,
                ));
            }
            "--dry-run" => dry_run = true,
            "--allow-partial" => allow_partial = true,
            other => bail!("unknown flag: {other}"),
        }
        i += 1;
    }

    if capture.is_some() && !online {
        bail!("--capture requires --online");
    }

    if dry_run {
        println!("gen-catalog fetch plan @ epoch JD {epoch} (TDB):");
        for row in plan(epoch) {
            println!("  {:<24} {:<9} {}", row.id, row.category, row.what);
        }
        return Ok(());
    }

    let opts = GenOptions {
        epoch_jd_tdb: epoch,
        allow_partial,
    };
    let invocation = format!(
        "cargo run -p xtask{} -- gen-catalog{}{} --epoch-jd {epoch}",
        if online { " --features online" } else { "" },
        if online { " --online" } else { "" },
        fixtures
            .as_ref()
            .map(|d| format!(" --fixtures {}", d.display()))
            .unwrap_or_default(),
    );
    let invocation = if let Some(dir) = &capture {
        format!("{invocation} --capture {}", dir.display())
    } else {
        invocation
    };

    let (catalog, skipped) = match (fixtures, online) {
        (Some(dir), false) => xtask::generate(&fetch::Fixtures { dir }, &opts)?,
        (None, true) => {
            #[cfg(feature = "online")]
            {
                xtask::generate(
                    &fetch::Http {
                        capture_dir: capture,
                    },
                    &opts,
                )?
            }
            #[cfg(not(feature = "online"))]
            bail!("--online requires: cargo run -p xtask --features online -- ...");
        }
        (Some(_), true) => bail!("choose one of --fixtures / --online"),
        (None, false) => bail!("choose a source: --fixtures DIR, --online, or --dry-run"),
    };

    emit::write_catalog(&catalog, &out, &invocation)?;
    println!(
        "wrote {} ({} bodies{})",
        out.display(),
        catalog.bodies.len(),
        if skipped.is_empty() {
            String::new()
        } else {
            format!("; skipped {}: {}", skipped.len(), skipped.join(", "))
        }
    );
    for l in catalog.lint() {
        println!("  lint: {l}");
    }
    Ok(())
}
