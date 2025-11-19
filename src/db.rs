// Copyright (c) The nextest Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Database operations for download statistics.

use anyhow::{Context, Result};
use camino::Utf8Path;
use chrono::NaiveDate;
use rusqlite::{Connection, params};

/// Initialize the database schema.
pub fn init_db(path: &Utf8Path) -> Result<Connection> {
    let conn = Connection::open(path.as_std_path())
        .with_context(|| format!("failed to open database at {}", path))?;

    // Enable SQLite best practices
    // Note: Some pragmas (like journal_mode, synchronous) persist in the database file.
    // Others (like cache_size, foreign_keys) are per-connection and must be set each time.
    conn.execute_batch(
        r#"
        -- WAL mode for better concurrency and crash recovery (PERSISTENT)
        -- WAL allows concurrent readers while writing and provides better crash recovery
        PRAGMA journal_mode = WAL;

        -- Synchronous mode: NORMAL is safe with WAL and much faster (PERSISTENT)
        -- FULL would be slower, OFF would be unsafe. NORMAL is the sweet spot with WAL.
        PRAGMA synchronous = NORMAL;

        -- Foreign key constraints enforcement (PER-CONNECTION)
        PRAGMA foreign_keys = ON;

        -- Increase cache size to 64MB (PER-CONNECTION, default is ~2MB)
        -- Negative value means size in KB, positive means number of pages
        PRAGMA cache_size = -64000;

        -- Use memory-mapped I/O for reads (PER-CONNECTION, 128MB)
        -- Speeds up read performance significantly
        PRAGMA mmap_size = 134217728;

        -- Enable automatic index creation for optimization (PER-CONNECTION)
        PRAGMA automatic_index = ON;

        -- Store temp tables in memory for better performance (PER-CONNECTION)
        PRAGMA temp_store = MEMORY;
        "#,
    )
    .context("failed to set database pragmas")?;

    // Create schema
    conn.execute_batch(
        r#"
        -- GitHub release asset downloads (snapshot-based)
        CREATE TABLE IF NOT EXISTS github_snapshots (
            date TEXT NOT NULL,              -- ISO8601 date (YYYY-MM-DD)
            release_tag TEXT NOT NULL,
            asset_name TEXT NOT NULL,
            download_count INTEGER NOT NULL,
            PRIMARY KEY (date, release_tag, asset_name)
        ) WITHOUT ROWID;  -- Optimization for tables with composite primary keys

        -- crates.io daily downloads (native time-series)
        CREATE TABLE IF NOT EXISTS crates_downloads (
            date TEXT NOT NULL,              -- ISO8601 date (YYYY-MM-DD)
            crate_name TEXT NOT NULL,
            version TEXT NOT NULL DEFAULT '', -- Empty string for aggregate stats
            downloads INTEGER NOT NULL,
            PRIMARY KEY (date, crate_name, version)
        ) WITHOUT ROWID;

        -- crates.io cumulative metadata snapshots
        CREATE TABLE IF NOT EXISTS crates_metadata (
            date TEXT NOT NULL,              -- ISO8601 date (YYYY-MM-DD)
            crate_name TEXT NOT NULL,
            total_downloads INTEGER NOT NULL,
            recent_downloads INTEGER NOT NULL,
            PRIMARY KEY (date, crate_name)
        ) WITHOUT ROWID;

        -- Computed weekly aggregates for graphing
        CREATE TABLE IF NOT EXISTS weekly_stats (
            week_start TEXT NOT NULL,        -- Monday of week (YYYY-MM-DD)
            source TEXT NOT NULL,            -- 'github' or 'crates'
            identifier TEXT NOT NULL,        -- crate name or 'releases'
            downloads INTEGER NOT NULL,
            PRIMARY KEY (week_start, source, identifier)
        ) WITHOUT ROWID;

        -- Indexes for efficient queries
        -- Note: PRIMARY KEY (date, ...) already provides an index on date, so no need for separate index
        CREATE INDEX IF NOT EXISTS idx_crates_crate ON crates_downloads(crate_name, date);
        CREATE INDEX IF NOT EXISTS idx_weekly_source ON weekly_stats(source, week_start);
        "#,
    )
    .context("failed to initialize database schema")?;

    Ok(conn)
}

/// Insert a GitHub release asset snapshot.
pub fn insert_github_snapshot(
    conn: &Connection,
    date: NaiveDate,
    release_tag: &str,
    asset_name: &str,
    download_count: u64,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO github_snapshots (date, release_tag, asset_name, download_count)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            date.to_string(),
            release_tag,
            asset_name,
            download_count as i64
        ],
    )
    .context("failed to insert GitHub snapshot")?;
    Ok(())
}

/// Insert a crates.io download record.
pub fn insert_crates_download(
    conn: &Connection,
    date: NaiveDate,
    crate_name: &str,
    version: Option<&str>,
    downloads: u64,
) -> Result<()> {
    let version_str = version.unwrap_or("");
    conn.execute(
        "INSERT OR REPLACE INTO crates_downloads (date, crate_name, version, downloads)
         VALUES (?1, ?2, ?3, ?4)",
        params![date.to_string(), crate_name, version_str, downloads as i64],
    )
    .context("failed to insert crates.io download")?;
    Ok(())
}

/// Insert a crates.io metadata snapshot.
pub fn insert_crates_metadata(
    conn: &Connection,
    date: NaiveDate,
    crate_name: &str,
    total_downloads: u64,
    recent_downloads: u64,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO crates_metadata (date, crate_name, total_downloads, recent_downloads)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            date.to_string(),
            crate_name,
            total_downloads as i64,
            recent_downloads as i64
        ],
    )
    .context("failed to insert crates.io metadata")?;
    Ok(())
}

/// Insert a weekly aggregate statistic.
pub fn insert_weekly_stat(
    conn: &Connection,
    week_start: NaiveDate,
    source: &str,
    identifier: &str,
    downloads: u64,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO weekly_stats (week_start, source, identifier, downloads)
         VALUES (?1, ?2, ?3, ?4)",
        params![week_start.to_string(), source, identifier, downloads as i64],
    )
    .context("failed to insert weekly stat")?;
    Ok(())
}

/// Get the latest date for which we have GitHub snapshots.
#[allow(dead_code)]
pub fn get_latest_github_snapshot_date(conn: &Connection) -> Result<Option<NaiveDate>> {
    let mut stmt = conn.prepare("SELECT MAX(date) FROM github_snapshots")?;
    let result: Option<String> = stmt.query_row([], |row| row.get(0))?;

    match result {
        Some(date_str) => {
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .context("failed to parse date from database")?;
            Ok(Some(date))
        }
        None => Ok(None),
    }
}

/// Get the latest date for which we have crates.io downloads.
#[allow(dead_code)]
pub fn get_latest_crates_download_date(
    conn: &Connection,
    crate_name: &str,
) -> Result<Option<NaiveDate>> {
    let mut stmt = conn.prepare("SELECT MAX(date) FROM crates_downloads WHERE crate_name = ?1")?;
    let result: Option<String> = stmt.query_row([crate_name], |row| row.get(0))?;

    match result {
        Some(date_str) => {
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .context("failed to parse date from database")?;
            Ok(Some(date))
        }
        None => Ok(None),
    }
}
