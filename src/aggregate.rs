// Copyright (c) The nextest Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Weekly aggregation of download statistics.

use crate::db;
use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate};
use rusqlite::Connection;
use std::collections::HashMap;

/// Get the Monday of the week containing the given date.
fn get_week_start(date: NaiveDate) -> NaiveDate {
    let weekday = date.weekday();
    let days_from_monday = weekday.num_days_from_monday();
    date - chrono::Duration::days(days_from_monday as i64)
}

/// Compute weekly aggregates for crates.io downloads.
///
/// This sums up daily downloads into weekly buckets (Monday-Sunday).
pub fn compute_crates_weekly(conn: &Connection) -> Result<()> {
    // Query all crates.io downloads
    let mut stmt = conn.prepare(
        "SELECT date, crate_name, SUM(downloads) as total
         FROM crates_downloads
         GROUP BY date, crate_name
         ORDER BY date",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    // Group by week and crate
    let mut weekly_data: HashMap<(NaiveDate, String), u64> = HashMap::new();

    for row in rows {
        let (date_str, crate_name, downloads) = row?;
        let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .with_context(|| format!("failed to parse date '{}'", date_str))?;
        let week_start = get_week_start(date);

        *weekly_data.entry((week_start, crate_name)).or_insert(0) += downloads as u64;
    }

    // Insert weekly aggregates
    for ((week_start, crate_name), downloads) in weekly_data {
        db::insert_weekly_stat(conn, week_start, "crates", &crate_name, downloads)?;
    }

    Ok(())
}

/// Compute weekly aggregates for GitHub release downloads.
///
/// Since GitHub only provides cumulative counts, we compute deltas between snapshots
/// and attribute them to the week of the later snapshot.
pub fn compute_github_weekly(conn: &Connection) -> Result<()> {
    // Query all GitHub snapshots ordered by date
    let mut stmt = conn.prepare(
        "SELECT date, release_tag, asset_name, download_count
         FROM github_snapshots
         ORDER BY release_tag, asset_name, date",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;

    // Track previous snapshot for each (release, asset) pair
    let mut prev_snapshots: HashMap<(String, String), (NaiveDate, i64)> = HashMap::new();
    let mut weekly_data: HashMap<NaiveDate, u64> = HashMap::new();

    for row in rows {
        let (date_str, release_tag, asset_name, download_count) = row?;
        let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .with_context(|| format!("failed to parse date '{}'", date_str))?;

        let key = (release_tag, asset_name);

        if let Some((_prev_date, prev_count)) = prev_snapshots.get(&key) {
            // Compute delta
            let delta = (download_count - prev_count).max(0) as u64;
            let week_start = get_week_start(date);

            *weekly_data.entry(week_start).or_insert(0) += delta;
        }

        // Update previous snapshot
        prev_snapshots.insert(key, (date, download_count));
    }

    // Insert weekly aggregates (using "releases" as the identifier)
    for (week_start, downloads) in weekly_data {
        db::insert_weekly_stat(conn, week_start, "github", "releases", downloads)?;
    }

    Ok(())
}

/// Compute all weekly aggregates.
pub fn compute_all_weekly(conn: &Connection) -> Result<()> {
    compute_crates_weekly(conn).context("failed to compute crates.io weekly aggregates")?;
    compute_github_weekly(conn).context("failed to compute GitHub weekly aggregates")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Weekday;

    #[test]
    fn test_get_week_start() {
        // 2025-11-19 is a Wednesday
        let date = NaiveDate::from_ymd_opt(2025, 11, 19).unwrap();
        let week_start = get_week_start(date);

        // Should return Monday of that week (2025-11-17)
        assert_eq!(week_start, NaiveDate::from_ymd_opt(2025, 11, 17).unwrap());
        assert_eq!(week_start.weekday(), Weekday::Mon);
    }

    #[test]
    fn test_get_week_start_already_monday() {
        // 2025-11-17 is a Monday
        let date = NaiveDate::from_ymd_opt(2025, 11, 17).unwrap();
        let week_start = get_week_start(date);

        // Should return itself
        assert_eq!(week_start, date);
    }
}
