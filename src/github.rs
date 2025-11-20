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

/// Fetch ALL releases from GitHub for a given repository using pagination.
///
/// This ensures we capture download stats for all releases, not just recent ones.
/// Old releases can continue getting downloads and we need to track that.
pub async fn fetch_releases(owner: &str, repo: &str) -> Result<Vec<Release>> {
    let client = reqwest::Client::new();
    let mut all_releases = Vec::new();
    let mut page = 1;
    let per_page = 100;

    let auth_header = std::env::var("GITHUB_TOKEN")
        .map(|token| format!("Bearer {}", token))
        .unwrap_or_default();

    loop {
        let url = format!(
            "{}/repos/{}/{}/releases?per_page={}&page={}",
            GITHUB_API_BASE, owner, repo, per_page, page
        );

        let response = client
            .get(&url)
            .header("User-Agent", "nextest-download-stats-collector")
            .header("Accept", "application/vnd.github.v3+json")
            .header("Authorization", &auth_header)
            .send()
            .await
            .with_context(|| format!("failed to fetch releases page {} from GitHub", page))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "GitHub API request failed with status {} on page {}: {}",
                status,
                page,
                body
            );
        }

        let releases: Vec<Release> = response
            .json()
            .await
            .with_context(|| format!("failed to parse GitHub API response for page {}", page))?;

        let is_last_page = releases.len() < per_page;
        all_releases.extend(releases);

        if is_last_page {
            break;
        }

        page += 1;
    }

    Ok(all_releases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_releases() {
        let releases = fetch_releases("nextest-rs", "nextest").await.unwrap();
        assert!(!releases.is_empty(), "should have at least one release");

        let has_assets = releases.iter().any(|r| !r.assets.is_empty());
        assert!(has_assets, "at least one release should have assets");
    }
}
