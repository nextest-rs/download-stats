// Copyright (c) The nextest Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Download statistics collector for nextest releases and crates.

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use chrono::Utc;
use clap::Parser;

mod aggregate;
mod config;
mod crates_io;
mod db;
mod github;
mod query;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the SQLite database file
    #[arg(short, long, default_value = "download-stats.db", global = true)]
    database: Utf8PathBuf,

    /// Path to the configuration file
    #[arg(short, long, default_value = "config.toml", global = true)]
    config: Utf8PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Parser, Debug)]
enum Command {
    /// Collect download statistics from GitHub and crates.io
    Collect {
        /// Skip GitHub release statistics collection
        #[arg(long)]
        skip_github: bool,

        /// Skip crates.io statistics collection
        #[arg(long)]
        skip_crates: bool,

        /// Skip weekly aggregation computation
        #[arg(long)]
        skip_aggregation: bool,
    },

    /// Query download statistics
    Query {
        #[command(subcommand)]
        query_type: QueryType,
    },

    /// Export statistics to various formats
    Export {
        #[command(subcommand)]
        export_type: ExportType,
    },
}

#[derive(Parser, Debug)]
enum QueryType {
    /// Show weekly download statistics
    Weekly {
        /// Number of weeks to show (default: 12)
        #[arg(short = 'n', long, default_value = "12")]
        limit: usize,

        /// Source to query: 'github', 'crates', or 'all'
        #[arg(short, long, default_value = "all")]
        source: String,
    },

    /// Show total downloads
    Total {
        /// Source to query: 'github', 'crates', or 'all'
        #[arg(short, long, default_value = "all")]
        source: String,
    },

    /// Show latest statistics
    Latest,
}

#[derive(Parser, Debug)]
enum ExportType {
    /// Export to CSV format
    Csv {
        /// Output file path
        #[arg(short, long)]
        output: Utf8PathBuf,

        /// What to export: 'weekly', 'daily', 'all'
        #[arg(short = 't', long, default_value = "weekly")]
        table: String,
    },

    /// Export to JSON format
    Json {
        /// Output file path
        #[arg(short, long)]
        output: Utf8PathBuf,

        /// What to export: 'weekly', 'daily', 'all'
        #[arg(short = 't', long, default_value = "weekly")]
        table: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Collect {
            skip_github,
            skip_crates,
            skip_aggregation,
        } => {
            let config = config::Config::load(&args.config)
                .context("failed to load configuration")?;
            run_collect(&args.database, &config, skip_github, skip_crates, skip_aggregation).await?;
        }
        Command::Query { query_type } => {
            let conn = db::init_db(&args.database).context("failed to open database")?;
            query::run_query(&conn, query_type)?;
        }
        Command::Export { export_type } => {
            let conn = db::init_db(&args.database).context("failed to open database")?;
            query::run_export(&conn, export_type)?;
        }
    }

    Ok(())
}

async fn run_collect(
    database: &camino::Utf8Path,
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
