# Technology Stack

**Analysis Date:** 2026-04-16

## Languages

**Primary:**
- Rust (edition 2021) - all source code

## Runtime

**Environment:**
- Native binary — no runtime required after compilation
- Targets local execution on developer machines (Linux/macOS)

**Package Manager:**
- Cargo (Rust built-in)
- Lockfile: `Cargo.lock` present (committed, 19 KB)

## Frameworks

**CLI:**
- `clap` 4 (derive feature) - argument parsing and subcommand dispatch (`src/main.rs`)
- `clap_complete` 4 - shell completion generation (bash/zsh/fish/etc.)

**Testing:**
- Rust built-in test framework (`cargo test`) - unit tests co-located in source files

**Build/Dev:**
- `cargo build --release` with aggressive optimization profile (see below)
- `cargo clippy` for linting (enforced in CI with `-D warnings`)

## Key Dependencies

**Critical:**
- `minreq` 2 (`json-using-serde` feature) - synchronous HTTP client; replaces previous `ureq`. No TLS support by design — all requests target localhost. (`src/client.rs`)
- `serde` 1 (derive feature) - serialization/deserialization framework (`src/types.rs`, `src/config.rs`)
- `serde_json` 1 - JSON encoding/decoding for API payloads and output (`src/client.rs`, `src/main.rs`, `src/merge.rs`)
- `anyhow` 1 - ergonomic error propagation throughout all modules

**Output / Display:**
- `tabled` 0.16 - terminal table rendering with `Style::modern()` (`src/output.rs`)
- `colored` 2 - ANSI color output for human-readable display (`src/main.rs`, `src/output.rs`)

**Config / Utility:**
- `toml` 0.8 - config file parsing/serialization (`src/config.rs`)
- `dirs` 5 - platform-aware config directory resolution (`~/.config/zotero-cli/`) (`src/config.rs`)
- `urlencoding` 2 - percent-encodes query strings for URL construction (`src/client.rs`)

## Configuration

**Environment:**
- Config file: `~/.config/zotero-cli/config.toml` (auto-created with defaults if absent)
- Env var overrides: `ZOTERO_API_BASE`, `ZOTERO_API_KEY`
- CLI flag override: `--api-base <URL>` (global flag)

**Config fields** (`src/config.rs`):
- `api_base` — default `http://localhost:23119/api`
- `api_key` — optional Zotero API key
- `user_id` — optional numeric user/group ID (0 = currently logged-in local user)
- `library_type` — `"user"` (default) or `"group"`

**Build:**
- Release profile in `Cargo.toml`: `strip = true`, `opt-level = "s"`, `lto = true`, `codegen-units = 1`
- Produces a minimal, stripped binary

## Platform Requirements

**Development:**
- Rust stable toolchain (resolved via `dtolnay/rust-toolchain@stable` in CI)
- Running Zotero desktop application with local connector active

**Production:**
- Single self-contained native binary
- No system dependencies beyond a running Zotero instance (for API calls)
- Zotero local connector must be listening on `localhost:23119` (or configured alternative)
- Zotero translation server (optional) must be on `localhost:1969` for `add url` subcommand

---

*Stack analysis: 2026-04-16*
