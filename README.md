# nextest download statistics collector

Automated collection of download statistics for nextest across multiple sources.

## Overview

This tool collects and aggregates download statistics from:

- **GitHub Releases**: Binary download counts for cargo-nextest releases
- **crates.io**: Daily download counts for all nextest crates

Data is stored in a SQLite database and automatically aggregated into weekly statistics for easy visualization and trend analysis.

## Architecture

### Data sources

#### GitHub releases API
- Provides **cumulative** download counts per release asset
- Limited to most recent 100 releases
- Sampled daily to compute download deltas over time

#### crates.io API
- Provides **daily** download counts (native time-series)
- Returns last year of historical data
- Tracked crates:
  - `cargo-nextest`
  - `nextest-runner`
  - `nextest-metadata`
  - `nextest-filtering`

### Database schema

```sql
-- GitHub release asset downloads (snapshot-based)
CREATE TABLE github_snapshots (
    date TEXT NOT NULL,              -- ISO8601 date (YYYY-MM-DD)
    release_tag TEXT NOT NULL,
    asset_name TEXT NOT NULL,
    download_count INTEGER NOT NULL,
    PRIMARY KEY (date, release_tag, asset_name)
);

-- crates.io daily downloads (native time-series)
CREATE TABLE crates_downloads (
    date TEXT NOT NULL,              -- ISO8601 date (YYYY-MM-DD)
    crate_name TEXT NOT NULL,
    version TEXT NOT NULL DEFAULT '', -- Empty string for aggregate stats
    downloads INTEGER NOT NULL,
    PRIMARY KEY (date, crate_name, version)
);

-- Computed weekly aggregates for graphing
CREATE TABLE weekly_stats (
    week_start TEXT NOT NULL,        -- Monday of week (YYYY-MM-DD)
    source TEXT NOT NULL,            -- 'github' or 'crates'
    identifier TEXT NOT NULL,        -- crate name or 'releases'
    downloads INTEGER NOT NULL,
    PRIMARY KEY (week_start, source, identifier)
);
```

## Usage

### Running locally

```bash
# Collect all statistics (creates/updates download-stats.db)
cargo run --release

# Skip specific sources
cargo run --release -- --skip-github
cargo run --release -- --skip-crates
cargo run --release -- --skip-aggregation

# Use custom database path
cargo run --release -- --database /path/to/stats.db
```

### Querying the database

```bash
# Weekly downloads for cargo-nextest crate
sqlite3 download-stats.db \
  "SELECT week_start, downloads FROM weekly_stats
   WHERE source = 'crates' AND identifier = 'cargo-nextest'
   ORDER BY week_start DESC LIMIT 10"

# Total GitHub release downloads by week
sqlite3 download-stats.db \
  "SELECT week_start, downloads FROM weekly_stats
   WHERE source = 'github'
   ORDER BY week_start DESC LIMIT 10"

# All crates combined per week
sqlite3 download-stats.db \
  "SELECT week_start, SUM(downloads) as total FROM weekly_stats
   WHERE source = 'crates'
   GROUP BY week_start
   ORDER BY week_start DESC LIMIT 10"
```

## Automated collection

A GitHub Actions workflow runs weekly (every Monday at 2 AM UTC) to:

1. Fetch latest statistics from both sources
2. Update the SQLite database
3. Compute weekly aggregates
4. Commit the updated database to the repository

The workflow can also be triggered manually via the Actions tab.

## Limitations

### GitHub releases
- API only provides cumulative counts (not time-series)
- Limited to most recent 100 releases
- Historical trends only available from when collection started
- No breakdown by platform/architecture

### crates.io
- Only provides last year of data
- Version field is a numeric ID, not semantic version
- Rate limit: 1 request per second

## Development

### Project structure

```
src/
├── main.rs        # CLI and orchestration
├── db.rs          # Database operations
├── github.rs      # GitHub API client
├── crates_io.rs   # crates.io API client
└── aggregate.rs   # Weekly aggregation logic
```

### Testing

```bash
# Run all tests
cargo test

# Check compilation
cargo check

# Run with test database
cargo run -- --database test-stats.db
```

## Future enhancements

Potential additions:

- **Visualization**: Generate static charts (PNG/SVG) from weekly data
- **Export**: CSV/JSON export for external analysis tools
- **Additional sources**:
  - Homebrew install counts
  - Docker Hub pulls
  - Package manager statistics (winget, etc.)
- **Incremental updates**: Only fetch new data since last run
- **Geographic distribution**: If APIs provide location data

## License

Licensed under either of:

- MIT License ([LICENSE-MIT](../nextest/LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](../nextest/LICENSE-APACHE))

at your option.
