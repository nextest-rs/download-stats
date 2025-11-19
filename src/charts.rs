// Copyright (c) The nextest Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Chart generation for download statistics visualization.

use anyhow::{Context, Result};
use camino::Utf8Path;
use chrono::NaiveDate;
use plotters::coord::types::RangedCoordi64;
use plotters::prelude::*;
use rusqlite::Connection;

const CHART_WIDTH: u32 = 1600;
const CHART_HEIGHT: u32 = 900;

// Typography - Inter font family
const FONT_FAMILY: &str = "Inter";
const TITLE_SIZE: i32 = 24;
const LABEL_SIZE: i32 = 16;
const AXIS_SIZE: i32 = 14;

// Colors - Modern, minimal palette
const BACKGROUND: RGBColor = RGBColor(250, 250, 252); // Off-white
const TEXT_PRIMARY: RGBColor = RGBColor(15, 23, 42); // Slate 900
const TEXT_SECONDARY: RGBColor = RGBColor(100, 116, 139); // Slate 500
const GRID_COLOR: RGBColor = RGBColor(226, 232, 240); // Slate 200
const ACCENT_BLUE: RGBColor = RGBColor(59, 130, 246); // Blue 500
const ACCENT_GREEN: RGBColor = RGBColor(34, 197, 94); // Green 500

/// Generate all charts from the database.
pub fn generate_all_charts(conn: &Connection, output_dir: &Utf8Path) -> Result<()> {
    std::fs::create_dir_all(output_dir.as_std_path())
        .with_context(|| format!("failed to create output directory at {}", output_dir))?;

    println!("\nGenerating charts...");

    generate_weekly_trends(conn, &output_dir.join("weekly-trends.png"))?;
    generate_cumulative_github(conn, &output_dir.join("github-cumulative.png"))?;
    generate_github_by_version(conn, &output_dir.join("github-by-version.png"))?;
    generate_source_comparison(conn, &output_dir.join("source-comparison.png"))?;

    println!("  ✓ Charts saved to {}", output_dir);
    Ok(())
}

/// Create a styled drawing area with background.
fn create_drawing_area(
    output_path: &Utf8Path,
) -> Result<DrawingArea<BitMapBackend<'_>, plotters::coord::Shift>> {
    let root = BitMapBackend::new(output_path.as_std_path(), (CHART_WIDTH, CHART_HEIGHT))
        .into_drawing_area();
    root.fill(&BACKGROUND)?;
    Ok(root)
}

/// Configure common mesh styling for date-based charts.
fn configure_date_mesh<DB: DrawingBackend>(
    chart: &mut ChartContext<DB, Cartesian2d<RangedDate<NaiveDate>, RangedCoordi64>>,
) -> Result<()>
where
    <DB as DrawingBackend>::ErrorType: 'static,
{
    chart
        .configure_mesh()
        .bold_line_style(&GRID_COLOR.mix(0.3))
        .light_line_style(&TRANSPARENT)
        .x_labels(8)
        .y_labels(6)
        .x_label_style((FONT_FAMILY, AXIS_SIZE).into_font().color(&TEXT_SECONDARY))
        .y_label_style((FONT_FAMILY, AXIS_SIZE).into_font().color(&TEXT_SECONDARY))
        .x_label_formatter(&|date| date.format("%Y-%m-%d").to_string())
        .y_label_formatter(&|y| format_number(*y as u64))
        .disable_x_mesh()
        .draw()?;
    Ok(())
}

/// Generate weekly download trends chart (line chart).
fn generate_weekly_trends(conn: &Connection, output_path: &Utf8Path) -> Result<()> {
    // Query weekly stats
    let mut stmt = conn.prepare(
        "SELECT week_start, SUM(downloads) as total
         FROM weekly_stats
         WHERE source = 'crates'
         GROUP BY week_start
         ORDER BY week_start ASC",
    )?;

    let data: Vec<(NaiveDate, i64)> = stmt
        .query_map([], |row| {
            let date_str: String = row.get(0)?;
            let downloads: i64 = row.get(1)?;
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            Ok((date, downloads))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if data.is_empty() {
        return Ok(());
    }

    let root = create_drawing_area(output_path)?;

    let min_date = data.first().unwrap().0;
    let max_date = data.last().unwrap().0;
    let max_downloads = data.iter().map(|(_, d)| *d).max().unwrap();

    let mut chart = ChartBuilder::on(&root)
        .caption(
            "Weekly Downloads - crates.io",
            (FONT_FAMILY, TITLE_SIZE).into_font().color(&TEXT_PRIMARY),
        )
        .margin(60)
        .x_label_area_size(70)
        .y_label_area_size(100)
        .build_cartesian_2d(min_date..max_date, 0i64..max_downloads)?;

    configure_date_mesh(&mut chart)?;

    chart.draw_series(LineSeries::new(
        data.iter().map(|(d, v)| (*d, *v)),
        ShapeStyle {
            color: ACCENT_BLUE.to_rgba(),
            filled: true,
            stroke_width: 3,
        },
    ))?;

    root.present()?;
    println!("  • weekly-trends.png");
    Ok(())
}

/// Generate cumulative GitHub downloads chart.
fn generate_cumulative_github(conn: &Connection, output_path: &Utf8Path) -> Result<()> {
    // Get GitHub snapshots over time
    let mut stmt = conn.prepare(
        "SELECT date, SUM(download_count) as total
         FROM github_snapshots
         GROUP BY date
         ORDER BY date ASC",
    )?;

    let data: Vec<(NaiveDate, i64)> = stmt
        .query_map([], |row| {
            let date_str: String = row.get(0)?;
            let downloads: i64 = row.get(1)?;
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            Ok((date, downloads))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if data.is_empty() {
        return Ok(());
    }

    let root = create_drawing_area(output_path)?;

    let min_date = data.first().unwrap().0;
    let max_date = data.last().unwrap().0;
    let max_downloads = data.iter().map(|(_, d)| *d).max().unwrap();

    let mut chart = ChartBuilder::on(&root)
        .caption(
            "Cumulative Downloads - GitHub Releases",
            (FONT_FAMILY, TITLE_SIZE).into_font().color(&TEXT_PRIMARY),
        )
        .margin(60)
        .x_label_area_size(70)
        .y_label_area_size(100)
        .build_cartesian_2d(min_date..max_date, 0i64..max_downloads)?;

    configure_date_mesh(&mut chart)?;

    chart.draw_series(AreaSeries::new(
        data.iter().map(|(d, v)| (*d, *v)),
        0,
        ACCENT_GREEN.mix(0.15),
    ))?;

    chart.draw_series(LineSeries::new(
        data.iter().map(|(d, v)| (*d, *v)),
        ShapeStyle {
            color: ACCENT_GREEN.to_rgba(),
            filled: true,
            stroke_width: 2,
        },
    ))?;

    root.present()?;
    println!("  • github-cumulative.png");
    Ok(())
}

/// Version info for chart categorization.
#[derive(Debug, Clone)]
struct VersionInfo {
    tag: String,
    version: semver::Version,
}

/// Generate GitHub downloads by version chart (stacked area).
fn generate_github_by_version(conn: &Connection, output_path: &Utf8Path) -> Result<()> {
    use std::collections::{HashMap, HashSet};

    // Get all cargo-nextest release tags with their download counts
    let mut tag_stmt = conn.prepare(
        "SELECT release_tag, SUM(download_count) as total
         FROM github_snapshots
         WHERE date = (SELECT MAX(date) FROM github_snapshots)
           AND release_tag LIKE 'cargo-nextest-%'
         GROUP BY release_tag
         ORDER BY release_tag DESC",
    )?;

    let all_tags: Vec<(String, i64)> = tag_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    if all_tags.is_empty() {
        return Ok(());
    }

    // Parse version numbers and sort by semantic version (latest first)
    let mut versions: Vec<(VersionInfo, i64)> = all_tags
        .into_iter()
        .filter_map(|(tag, downloads)| {
            tag.strip_prefix("cargo-nextest-")
                .and_then(|v| semver::Version::parse(v).ok())
                .map(|version| (VersionInfo { tag, version }, downloads))
        })
        .collect();

    versions.sort_by(|a, b| b.0.version.cmp(&a.0.version)); // Sort descending (latest first)

    // Filter out versions with trivial downloads, then take the 5 most recent
    // Heuristic: must have at least 10,000 downloads or 0.5% of max
    let max_downloads = versions.iter().map(|(_, d)| *d).max().unwrap_or(0);
    let threshold = (max_downloads as f64 * 0.005).max(10_000.0) as i64;

    // Keep structured data: already sorted by semver descending
    let top_versions: Vec<VersionInfo> = versions
        .into_iter()
        .filter(|(_, downloads)| *downloads >= threshold)
        .take(5)
        .map(|(info, _)| info)
        .collect();

    let top_tags: HashSet<&str> = top_versions.iter().map(|v| v.tag.as_str()).collect();

    // Query all snapshots
    let mut stmt = conn.prepare(
        "SELECT date, release_tag, SUM(download_count) as total
         FROM github_snapshots
         GROUP BY date, release_tag
         ORDER BY date ASC, release_tag ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        let date_str: String = row.get(0)?;
        let tag: String = row.get(1)?;
        let downloads: i64 = row.get(2)?;
        let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        Ok((date, tag, downloads))
    })?;

    // Organize data by date and version
    let mut data_by_date: HashMap<NaiveDate, HashMap<String, i64>> = HashMap::new();
    let mut all_dates: HashSet<NaiveDate> = HashSet::new();

    for row in rows {
        let (date, tag, downloads) = row?;
        all_dates.insert(date);

        let category = if top_tags.contains(tag.as_str()) {
            tag
        } else {
            "Other".to_string()
        };

        *data_by_date
            .entry(date)
            .or_default()
            .entry(category)
            .or_default() += downloads;
    }

    if all_dates.is_empty() {
        return Ok(());
    }

    let mut dates: Vec<NaiveDate> = all_dates.into_iter().collect();
    dates.sort();

    // Create series data - categories are already sorted by semver descending
    let mut series_data: HashMap<String, Vec<(NaiveDate, i64)>> = HashMap::new();
    let mut categories: Vec<String> = top_versions.iter().map(|v| v.tag.clone()).collect();
    categories.push("Other".to_string());

    for &date in &dates {
        for category in &categories {
            let value = data_by_date
                .get(&date)
                .and_then(|m| m.get(category))
                .copied()
                .unwrap_or(0);
            series_data
                .entry(category.clone())
                .or_default()
                .push((date, value));
        }
    }

    let root = create_drawing_area(output_path)?;

    let min_date = *dates.first().unwrap();
    let max_date = *dates.last().unwrap();
    let max_downloads = data_by_date
        .values()
        .map(|m| m.values().sum::<i64>())
        .max()
        .unwrap();

    let mut chart = ChartBuilder::on(&root)
        .caption(
            "Cumulative Downloads by Version - GitHub Releases",
            (FONT_FAMILY, TITLE_SIZE).into_font().color(&TEXT_PRIMARY),
        )
        .margin(60)
        .x_label_area_size(70)
        .y_label_area_size(100)
        .build_cartesian_2d(min_date..max_date, 0i64..max_downloads)?;

    configure_date_mesh(&mut chart)?;

    // Color palette for versions
    let colors = [
        RGBColor(99, 102, 241),  // Indigo
        RGBColor(59, 130, 246),  // Blue
        RGBColor(34, 197, 94),   // Green
        RGBColor(251, 146, 60),  // Orange
        RGBColor(236, 72, 153),  // Pink
        RGBColor(156, 163, 175), // Gray (for "Other")
    ];

    // Draw stacked areas
    for (idx, category) in categories.iter().enumerate() {
        if let Some(data) = series_data.get(category) {
            let color = colors[idx % colors.len()];
            chart.draw_series(AreaSeries::new(
                data.iter().map(|(d, v)| (*d, *v)),
                0,
                color.mix(0.3),
            ))?;

            chart
                .draw_series(LineSeries::new(
                    data.iter().map(|(d, v)| (*d, *v)),
                    ShapeStyle {
                        color: color.to_rgba(),
                        filled: true,
                        stroke_width: 2,
                    },
                ))?
                .label(category)
                .legend(move |(x, y)| {
                    Rectangle::new([(x, y - 5), (x + 15, y + 5)], color.filled())
                });
        }
    }

    chart
        .configure_series_labels()
        .label_font((FONT_FAMILY, LABEL_SIZE).into_font().color(&TEXT_PRIMARY))
        .background_style(&BACKGROUND)
        .border_style(&GRID_COLOR)
        .margin(15)
        .draw()?;

    root.present()?;
    println!("  • github-by-version.png");
    Ok(())
}

/// Generate source comparison chart (GitHub vs crates.io).
fn generate_source_comparison(conn: &Connection, output_path: &Utf8Path) -> Result<()> {
    // Get weekly stats by source
    let mut stmt = conn.prepare(
        "SELECT week_start, source, SUM(downloads) as total
         FROM weekly_stats
         GROUP BY week_start, source
         ORDER BY week_start ASC, source ASC",
    )?;

    let mut crates_data: Vec<(NaiveDate, i64)> = Vec::new();
    let mut github_data: Vec<(NaiveDate, i64)> = Vec::new();

    let rows = stmt.query_map([], |row| {
        let date_str: String = row.get(0)?;
        let source: String = row.get(1)?;
        let downloads: i64 = row.get(2)?;
        let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        Ok((date, source, downloads))
    })?;

    for row in rows {
        let (date, source, downloads) = row?;
        match source.as_str() {
            "crates" => crates_data.push((date, downloads)),
            "github" => github_data.push((date, downloads)),
            _ => {}
        }
    }

    if crates_data.is_empty() && github_data.is_empty() {
        return Ok(());
    }

    let root = create_drawing_area(output_path)?;

    let all_dates: Vec<_> = crates_data
        .iter()
        .chain(github_data.iter())
        .map(|(d, _)| *d)
        .collect();
    let min_date = *all_dates.iter().min().unwrap();
    let max_date = *all_dates.iter().max().unwrap();

    let max_downloads = crates_data
        .iter()
        .chain(github_data.iter())
        .map(|(_, d)| *d)
        .max()
        .unwrap();

    let mut chart = ChartBuilder::on(&root)
        .caption(
            "Weekly Downloads by Source",
            (FONT_FAMILY, TITLE_SIZE).into_font().color(&TEXT_PRIMARY),
        )
        .margin(60)
        .x_label_area_size(70)
        .y_label_area_size(100)
        .build_cartesian_2d(min_date..max_date, 0i64..max_downloads)?;

    configure_date_mesh(&mut chart)?;

    if !crates_data.is_empty() {
        chart
            .draw_series(LineSeries::new(
                crates_data.iter().map(|(d, v)| (*d, *v)),
                ShapeStyle {
                    color: ACCENT_BLUE.to_rgba(),
                    filled: true,
                    stroke_width: 3,
                },
            ))?
            .label("crates.io")
            .legend(|(x, y)| Rectangle::new([(x, y - 5), (x + 15, y + 5)], ACCENT_BLUE.filled()));
    }

    if !github_data.is_empty() {
        chart
            .draw_series(LineSeries::new(
                github_data.iter().map(|(d, v)| (*d, *v)),
                ShapeStyle {
                    color: ACCENT_GREEN.to_rgba(),
                    filled: true,
                    stroke_width: 3,
                },
            ))?
            .label("GitHub")
            .legend(|(x, y)| Rectangle::new([(x, y - 5), (x + 15, y + 5)], ACCENT_GREEN.filled()));
    }

    chart
        .configure_series_labels()
        .label_font((FONT_FAMILY, LABEL_SIZE).into_font().color(&TEXT_PRIMARY))
        .background_style(&BACKGROUND)
        .border_style(&GRID_COLOR)
        .margin(15)
        .draw()?;

    root.present()?;
    println!("  • source-comparison.png");
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
