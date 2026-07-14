//! CLI:
//!   cargo run -p xtask -- gen-catalog --dry-run
//!   cargo run -p xtask -- gen-catalog --fixtures xtask/fixtures --allow-partial --out assets/catalog.sample.ron
//!   cargo run -p xtask --features online -- gen-catalog --online --capture xtask/fixtures/captured-YYYY-MM --out assets/catalog.ron
//!   cargo run -p xtask -- bake-starfield --source bsc5p.vot --out assets/starfield.bsc

use anyhow::{bail, Result};
use std::path::PathBuf;
use xtask::{emit, fetch, plan, starfield, GenOptions, DEFAULT_EPOCH_JD_TDB};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("gen-catalog") => gen_catalog(&args[1..]),
        Some("bake-starfield") => bake_starfield(&args[1..]),
        _ => {
            eprintln!("usage:\n  xtask gen-catalog [--out PATH] [--epoch-jd F] [--dry-run] [--fixtures DIR [--allow-partial]] [--online [--capture DIR]]\n  xtask bake-starfield --source PATH --out PATH [--limit N]");
            std::process::exit(2);
        }
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
