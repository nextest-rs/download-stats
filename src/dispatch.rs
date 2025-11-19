// Copyright (c) The nextest Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! CLI argument parsing and command dispatch.

use crate::{commands, config, db, query};
use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
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

    /// Generate charts from collected statistics
    Charts {
        /// Output directory for charts
        #[arg(short, long, default_value = "charts")]
        output: Utf8PathBuf,
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

/// Parse arguments and dispatch to the appropriate command.
pub async fn dispatch() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Collect {
            skip_github,
            skip_crates,
            skip_aggregation,
        } => {
            let config =
                config::Config::load(&args.config).context("failed to load configuration")?;
            commands::run_collect(
                &args.database,
                &config,
                skip_github,
                skip_crates,
                skip_aggregation,
            )
            .await?;
        }
        Command::Charts { output } => {
            commands::run_charts(&args.database, &output)?;
        }
        Command::Query { query_type } => {
            let conn = db::init_db(&args.database).context("failed to open database")?;
            let query_kind = match query_type {
                QueryType::Weekly { limit, source } => query::QueryKind::Weekly { limit, source },
                QueryType::Total { source } => query::QueryKind::Total { source },
                QueryType::Latest => query::QueryKind::Latest,
            };
            query::run_query(&conn, query_kind)?;
        }
        Command::Export { export_type } => {
            let conn = db::init_db(&args.database).context("failed to open database")?;
            let export_kind = match export_type {
                ExportType::Csv { output, table } => query::ExportKind::Csv {
                    output: output.to_string(),
                    table,
                },
                ExportType::Json { output, table } => query::ExportKind::Json {
                    output: output.to_string(),
                    table,
                },
            };
            query::run_export(&conn, export_kind)?;
        }
    }

    Ok(())
}
