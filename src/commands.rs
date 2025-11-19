// Copyright (c) The nextest Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Command implementations.

use anyhow::{Context, Result};
use camino::Utf8Path;
use chrono::Utc;

use crate::{aggregate, charts, config, crates_io, db, github};

/// Run the collect command.
pub async fn run_collect(
    database: &Utf8Path,
    config: &config::Config,
    skip_github: bool,
    skip_crates: bool,
    skip_aggregation: bool,
) -> Result<()> {
    println!("Initializing database at {}", database);
    let conn = db::init_db(database).context("failed to initialize database")?;

    let today = Utc::now().date_naive();

    if !skip_github {
        println!("\nCollecting GitHub release statistics...");
        for (owner, repo) in config.github_sources() {
            println!("  {}/{}", owner, repo);
            collect_github_stats(&conn, today, owner, repo).await?;
        }
    }

    if !skip_crates {
        println!("\nCollecting crates.io statistics...");
        for crate_name in config.crates_sources() {
            println!("  {}", crate_name);
            collect_crates_stats(&conn, crate_name).await?;
        }
    }

    if !skip_aggregation {
        println!("\nComputing weekly aggregates...");
        aggregate::compute_all_weekly(&conn)?;
    }

    println!("\n✓ Collection complete!");
    Ok(())
}

/// Run the charts command.
pub fn run_charts(database: &Utf8Path, output_dir: &Utf8Path) -> Result<()> {
    let conn = db::init_db(database).context("failed to open database")?;
    charts::generate_all_charts(&conn, output_dir)?;
    Ok(())
}

async fn collect_github_stats(
    conn: &rusqlite::Connection,
    today: chrono::NaiveDate,
    owner: &str,
    repo: &str,
) -> Result<()> {
    let releases = github::fetch_releases(owner, repo)
        .await
        .context("failed to fetch GitHub releases")?;

    println!("  Found {} releases", releases.len());

    let mut total_assets = 0;
    let mut total_downloads = 0;

    for release in releases {
        // Only collect cargo-nextest releases
        if !release.tag_name.starts_with("cargo-nextest-") {
            continue;
        }

        for asset in release.assets {
            db::insert_github_snapshot(
                conn,
                today,
                &release.tag_name,
                &asset.name,
                asset.download_count,
            )?;
            total_assets += 1;
            total_downloads += asset.download_count;
        }
    }

    println!(
        "  Recorded {} assets with {} total downloads",
        total_assets, total_downloads
    );
    Ok(())
}

async fn collect_crates_stats(conn: &rusqlite::Connection, crate_name: &str) -> Result<()> {
    let downloads = crates_io::fetch_downloads(crate_name)
        .await
        .with_context(|| format!("failed to fetch downloads for '{}'", crate_name))?;

    let mut records_inserted = 0;

    // Insert version-specific downloads
    for vd in downloads.version_downloads {
        let date = crates_io::parse_date(&vd.date)?;
        let version_str = vd.version.to_string();
        db::insert_crates_download(conn, date, crate_name, Some(&version_str), vd.downloads)?;
        records_inserted += 1;
    }

    // Insert aggregate downloads (for versions without detailed tracking)
    for ed in downloads.meta.extra_downloads {
        let date = crates_io::parse_date(&ed.date)?;
        db::insert_crates_download(conn, date, crate_name, None, ed.downloads)?;
        records_inserted += 1;
    }

    println!("    Inserted {} records", records_inserted);
    Ok(())
}
