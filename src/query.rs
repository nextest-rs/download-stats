// Copyright (c) The nextest Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Query and export functionality for download statistics.

use anyhow::{Context, Result};
use camino::Utf8Path;
use rusqlite::Connection;
use std::{fs::File, io::Write};

pub enum QueryKind {
    Weekly { limit: usize, source: String },
    Total { source: String },
    Latest,
}

pub enum ExportKind {
    Csv { output: String, table: String },
    Json { output: String, table: String },
}

pub fn run_query(conn: &Connection, query: QueryKind) -> Result<()> {
    match query {
        QueryKind::Weekly { limit, source } => query_weekly(conn, limit, &source)?,
        QueryKind::Total { source } => query_total(conn, &source)?,
        QueryKind::Latest => query_latest(conn)?,
    }
    Ok(())
}

pub fn run_export(conn: &Connection, export: ExportKind) -> Result<()> {
    match export {
        ExportKind::Csv { output, table } => export_csv(conn, output.as_ref(), &table)?,
        ExportKind::Json { output, table } => export_json(conn, output.as_ref(), &table)?,
    }
    Ok(())
}

fn query_weekly(conn: &Connection, limit: usize, source: &str) -> Result<()> {
    let query = match source {
        "github" => {
            "SELECT week_start, downloads FROM weekly_stats
             WHERE source = 'github'
             ORDER BY week_start DESC LIMIT ?1"
        }
        "crates" => {
            "SELECT week_start, SUM(downloads) as downloads FROM weekly_stats
             WHERE source = 'crates'
             GROUP BY week_start
             ORDER BY week_start DESC LIMIT ?1"
        }
        "all" | _ => {
            "SELECT week_start, SUM(downloads) as downloads FROM weekly_stats
             GROUP BY week_start
             ORDER BY week_start DESC LIMIT ?1"
        }
    };

    let mut stmt = conn.prepare(query)?;
    let rows = stmt.query_map([limit], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;

    println!("\n{:<12} {:>15}", "Week", "Downloads");
    println!("{}", "=".repeat(30));

    for row in rows {
        let (week, downloads) = row?;
        println!("{:<12} {:>15}", week, format_number(downloads as u64));
    }

    Ok(())
}

fn query_total(conn: &Connection, source: &str) -> Result<()> {
    let (total_downloads, description) = match source {
        "github" => {
            let total: i64 = conn.query_row(
                "SELECT SUM(downloads) FROM weekly_stats WHERE source = 'github'",
                [],
                |row| row.get(0),
            )?;
            (total, "GitHub releases (tracked period)")
        }
        "crates" => {
            let total: i64 = conn.query_row(
                "SELECT SUM(downloads) FROM weekly_stats WHERE source = 'crates'",
                [],
                |row| row.get(0),
            )?;
            (total, "crates.io (last year)")
        }
        "all" | _ => {
            let total: i64 =
                conn.query_row("SELECT SUM(downloads) FROM weekly_stats", [], |row| {
                    row.get(0)
                })?;
            (total, "All sources")
        }
    };

    println!("\nTotal downloads");
    println!("  Source: {}", description);
    println!("  Total:  {}", format_number(total_downloads as u64));

    Ok(())
}

fn query_latest(conn: &Connection) -> Result<()> {
    println!("\nLatest statistics\n");

    let (latest_week, crates_downloads): (String, i64) = conn.query_row(
        "SELECT week_start, SUM(downloads) FROM weekly_stats
         WHERE source = 'crates'
         GROUP BY week_start
         ORDER BY week_start DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    println!("Latest week: {}", latest_week);
    println!("  crates.io: {}", format_number(crates_downloads as u64));

    let github_total: i64 = conn.query_row(
        "SELECT SUM(download_count) FROM github_snapshots
         WHERE date = (SELECT MAX(date) FROM github_snapshots)",
        [],
        |row| row.get(0),
    )?;

    println!(
        "  GitHub (cumulative): {}",
        format_number(github_total as u64)
    );

    let (first_week, last_week): (String, String) = conn.query_row(
        "SELECT MIN(week_start), MAX(week_start) FROM weekly_stats",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    println!("\nData coverage: {} to {}", first_week, last_week);

    Ok(())
}

fn export_csv(conn: &Connection, output: &Utf8Path, table: &str) -> Result<()> {
    let query = match table {
        "weekly" => "SELECT * FROM weekly_stats ORDER BY week_start, source, identifier",
        "daily" => "SELECT * FROM crates_downloads ORDER BY date, crate_name, version",
        "github" => "SELECT * FROM github_snapshots ORDER BY date, release_tag, asset_name",
        _ => anyhow::bail!(
            "Unknown table type: {}. Use 'weekly', 'daily', or 'github'",
            table
        ),
    };

    let mut stmt = conn.prepare(query)?;
    let column_count = stmt.column_count();
    let column_names: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();

    let mut file = File::create(output.as_std_path())
        .with_context(|| format!("failed to create file at {}", output))?;

    writeln!(file, "{}", column_names.join(","))?;

    let rows = stmt.query_map([], |row| {
        let mut values = Vec::new();
        for i in 0..column_count {
            let value = match row.get_ref(i)? {
                rusqlite::types::ValueRef::Null => String::new(),
                rusqlite::types::ValueRef::Integer(i) => i.to_string(),
                rusqlite::types::ValueRef::Real(f) => f.to_string(),
                rusqlite::types::ValueRef::Text(s) => {
                    std::str::from_utf8(s).unwrap_or("").to_string()
                }
                rusqlite::types::ValueRef::Blob(b) => format!("{:?}", b),
            };
            values.push(value);
        }
        Ok(values)
    })?;

    for row in rows {
        let values = row?;
        writeln!(file, "{}", values.join(","))?;
    }

    println!("Exported to {}.", output);
    Ok(())
}

fn export_json(conn: &Connection, output: &Utf8Path, table: &str) -> Result<()> {
    let query = match table {
        "weekly" => "SELECT * FROM weekly_stats ORDER BY week_start, source, identifier",
        "daily" => "SELECT * FROM crates_downloads ORDER BY date, crate_name, version",
        "github" => "SELECT * FROM github_snapshots ORDER BY date, release_tag, asset_name",
        _ => anyhow::bail!(
            "Unknown table type: {}. Use 'weekly', 'daily', or 'github'",
            table
        ),
    };

    let mut stmt = conn.prepare(query)?;
    let column_names: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();

    let rows = stmt.query_map([], |row| {
        let mut map = serde_json::Map::new();
        for (i, name) in column_names.iter().enumerate() {
            let value = match row.get_ref(i)? {
                rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                rusqlite::types::ValueRef::Integer(n) => serde_json::Value::Number(n.into()),
                rusqlite::types::ValueRef::Real(f) => serde_json::Number::from_f64(f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
                rusqlite::types::ValueRef::Text(s) => {
                    serde_json::Value::String(std::str::from_utf8(s).unwrap_or("").to_string())
                }
                rusqlite::types::ValueRef::Blob(b) => serde_json::Value::String(format!("{:?}", b)),
            };
            map.insert(name.clone(), value);
        }
        Ok(serde_json::Value::Object(map))
    })?;

    let mut records = Vec::new();
    for row in rows {
        records.push(row?);
    }

    let json = serde_json::to_string_pretty(&records)?;

    let mut file = File::create(output.as_std_path())
        .with_context(|| format!("failed to create file at {}", output))?;
    file.write_all(json.as_bytes())?;

    println!("Exported to {}.", output);
    Ok(())
}

/// Format a number with thousands separators.
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let mut count = 0;

    for c in s.chars().rev() {
        if count > 0 && count % 3 == 0 {
            result.push(',');
        }
        result.push(c);
        count += 1;
    }

    result.chars().rev().collect()
}
