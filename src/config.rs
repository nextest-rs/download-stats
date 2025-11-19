// Copyright (c) The nextest Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Configuration for download statistics collection.

use anyhow::{Context, Result};
use camino::Utf8Path;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub source: Vec<CollectionSource>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CollectionSource {
    Github {
        owner: String,
        repo: String,
    },
    Crates {
        name: String,
    },
}

impl Config {
    /// Load configuration from a TOML file.
    pub fn load(path: &Utf8Path) -> Result<Self> {
        let content = fs::read_to_string(path.as_std_path())
            .with_context(|| format!("failed to read config file at {}", path))?;

        toml::from_str(&content)
            .with_context(|| format!("failed to parse config file at {}", path))
    }

    /// Get all GitHub sources.
    pub fn github_sources(&self) -> impl Iterator<Item = (&str, &str)> {
        self.source.iter().filter_map(|s| match s {
            CollectionSource::Github { owner, repo } => Some((owner.as_str(), repo.as_str())),
            _ => None,
        })
    }

    /// Get all crates.io sources.
    pub fn crates_sources(&self) -> impl Iterator<Item = &str> {
        self.source.iter().filter_map(|s| match s {
            CollectionSource::Crates { name } => Some(name.as_str()),
            _ => None,
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            source: vec![
                CollectionSource::Github {
                    owner: "nextest-rs".to_string(),
                    repo: "nextest".to_string(),
                },
                CollectionSource::Crates {
                    name: "cargo-nextest".to_string(),
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_roundtrip() {
        let config = Config::default();
        let toml = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml).unwrap();

        assert_eq!(config.source.len(), parsed.source.len());
    }

    #[test]
    fn test_parse_config() {
        let toml = r#"
[[source]]
kind = "github"
owner = "nextest-rs"
repo = "nextest"

[[source]]
kind = "crates"
name = "cargo-nextest"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.source.len(), 2);

        let github: Vec<_> = config.github_sources().collect();
        assert_eq!(github.len(), 1);
        assert_eq!(github[0], ("nextest-rs", "nextest"));

        let crates: Vec<_> = config.crates_sources().collect();
        assert_eq!(crates.len(), 1);
        assert_eq!(crates[0], "cargo-nextest");
    }
}
