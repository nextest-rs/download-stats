// Copyright (c) The nextest Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! GitHub API client for fetching release download statistics.

use anyhow::{Context, Result};
use serde::Deserialize;

const GITHUB_API_BASE: &str = "https://api.github.com";

#[derive(Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub download_count: u64,
}

/// Fetch releases from GitHub for a given repository.
///
/// Note: The GitHub API only returns the most recent 30 releases by default.
/// We fetch 100 to get better coverage, though this is still limited.
pub async fn fetch_releases(owner: &str, repo: &str) -> Result<Vec<Release>> {
    let url = format!(
        "{}/repos/{}/{}/releases?per_page=100",
        GITHUB_API_BASE, owner, repo
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("User-Agent", "nextest-download-stats-collector")
        .header("Accept", "application/vnd.github.v3+json")
        // Use token if available in environment
        .header(
            "Authorization",
            std::env::var("GITHUB_TOKEN")
                .map(|token| format!("Bearer {}", token))
                .unwrap_or_default(),
        )
        .send()
        .await
        .context("failed to fetch releases from GitHub")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("GitHub API request failed with status {}: {}", status, body);
    }

    let releases = response
        .json::<Vec<Release>>()
        .await
        .context("failed to parse GitHub API response")?;

    Ok(releases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_releases() {
        // This test requires network access
        let releases = fetch_releases("nextest-rs", "nextest").await.unwrap();
        assert!(!releases.is_empty(), "should have at least one release");

        // Check that we got assets with download counts
        let has_assets = releases.iter().any(|r| !r.assets.is_empty());
        assert!(has_assets, "at least one release should have assets");
    }
}
