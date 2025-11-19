// Copyright (c) The nextest Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! crates.io API client for fetching download statistics.

use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::Deserialize;

const CRATES_IO_API_BASE: &str = "https://crates.io/api/v1";

#[derive(Debug, Deserialize)]
pub struct CrateResponse {
    #[serde(rename = "crate")]
    pub crate_info: CrateInfo,
}

#[derive(Debug, Deserialize)]
pub struct CrateInfo {
    pub downloads: u64,
    pub recent_downloads: u64,
}

#[derive(Debug, Deserialize)]
pub struct DownloadsResponse {
    pub version_downloads: Vec<VersionDownload>,
    pub meta: DownloadsMeta,
}

#[derive(Debug, Deserialize)]
pub struct VersionDownload {
    pub version: u64, // Numeric version ID from crates.io
    pub downloads: u64,
    pub date: String, // YYYY-MM-DD format
}

#[derive(Debug, Deserialize)]
pub struct DownloadsMeta {
    pub extra_downloads: Vec<ExtraDownload>,
}

#[derive(Debug, Deserialize)]
pub struct ExtraDownload {
    pub date: String, // YYYY-MM-DD format
    pub downloads: u64,
}

/// Fetch crate metadata including cumulative download totals.
pub async fn fetch_crate_metadata(crate_name: &str) -> Result<CrateInfo> {
    let url = format!("{}/crates/{}", CRATES_IO_API_BASE, crate_name);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header(
            "User-Agent",
            "nextest-download-stats-collector (contact: opensource@nexte.st)",
        )
        .send()
        .await
        .with_context(|| format!("failed to fetch metadata for crate '{}'", crate_name))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "crates.io API request failed with status {} for crate '{}': {}",
            status,
            crate_name,
            body
        );
    }

    let crate_response = response
        .json::<CrateResponse>()
        .await
        .context("failed to parse crates.io API response")?;

    Ok(crate_response.crate_info)
}

/// Fetch download statistics for a crate from crates.io.
///
/// Note: The crates.io API only provides the last year of data.
pub async fn fetch_downloads(crate_name: &str) -> Result<DownloadsResponse> {
    let url = format!("{}/crates/{}/downloads", CRATES_IO_API_BASE, crate_name);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header(
            "User-Agent",
            "nextest-download-stats-collector (contact: opensource@nexte.st)",
        )
        .send()
        .await
        .with_context(|| format!("failed to fetch downloads for crate '{}'", crate_name))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "crates.io API request failed with status {} for crate '{}': {}",
            status,
            crate_name,
            body
        );
    }

    let downloads = response
        .json::<DownloadsResponse>()
        .await
        .context("failed to parse crates.io API response")?;

    Ok(downloads)
}

/// Parse a date string from crates.io (YYYY-MM-DD format).
pub fn parse_date(date_str: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .with_context(|| format!("failed to parse date '{}'", date_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[tokio::test]
    async fn test_fetch_downloads() {
        // This test requires network access
        let downloads = fetch_downloads("cargo-nextest").await.unwrap();
        assert!(
            !downloads.version_downloads.is_empty(),
            "should have version downloads"
        );
        assert!(
            !downloads.meta.extra_downloads.is_empty(),
            "should have extra downloads"
        );

        // Verify date parsing
        for vd in &downloads.version_downloads {
            parse_date(&vd.date).expect("should parse date");
        }
    }

    #[test]
    fn test_parse_date() {
        let date = parse_date("2025-11-19").unwrap();
        assert_eq!(date.year(), 2025);
        assert_eq!(date.month(), 11);
        assert_eq!(date.day(), 19);
    }
}
