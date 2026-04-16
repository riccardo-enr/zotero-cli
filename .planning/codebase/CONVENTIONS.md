# Coding Conventions

**Analysis Date:** 2026-04-16

## Naming Patterns

**Files:**
- `snake_case.rs` for all source files: `client.rs`, `config.rs`, `merge.rs`, `output.rs`, `types.rs`
- One module per file; module name matches filename

**Functions:**
- `snake_case` for all functions: `get_json`, `patch_json`, `lib_path`, `display_name`, `items_table`, `build_dry_run_report`
- Private helpers at the bottom of the file after public API
- Constructor convention: `Type::new(cfg)` for structs requiring initialization, `Type::load()` for deserialization from disk

**Variables:**
- `snake_case` throughout
- Short, single-purpose bindings preferred: `lib`, `url`, `body`, `resp`

**Types:**
- `PascalCase` structs: `ZoteroItem`, `ZoteroClient`, `ItemData`, `CompactItem`, `CollectionData`
- Enum variants `PascalCase`: `Commands::Search`, `AddKind::Doi`
- Constants `SCREAMING_SNAKE_CASE`: `API_VERSION`, `TRANSLATOR_URL`, `STRUCTURAL_FIELDS`, `TITLE_MAX_WIDTH`

**Test functions:**
- Descriptive `snake_case` names that read as sentences: `display_name_first_and_last`, `prefer_nonempty_target`, `fill_empty_from_source`, `structural_fields_unchanged`

## Code Style

**Formatting:**
- Standard `rustfmt` defaults (no `rustfmt.toml` present)
- 4-space indentation
- Trailing commas in multi-line struct/enum/function-call contexts
- Opening brace on same line as `fn`/`impl`/`struct`

**Linting:**
- `clippy` with `-D warnings` (all clippy warnings treated as errors in CI)
- No `clippy.toml`; uses clippy defaults

## Import Organization

**Order (in each file):**
1. Standard library (`std::...`)
2. External crates (`anyhow`, `serde_json`, `colored`, etc.)
3. Blank line
4. Internal crate modules (`crate::config::Config`, `crate::types::...`)

**Examples from source:**
```rust
// client.rs
use anyhow::{Context, Result};
use serde_json::Value;
use urlencoding::encode;

use crate::config::Config;
use crate::types::{ZoteroCollection, ZoteroItem};
```

```rust
// main.rs
use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use colored::Colorize;

use client::ZoteroClient;
use config::Config;
use types::CompactItem;
```

**Path Aliases:**
- None used; all imports are fully qualified

## Error Handling

**Framework:** `anyhow` for all error propagation and error construction

**Patterns:**
- All fallible functions return `Result<T>` (anyhow's `Result`)
- Use `.context("message")` to annotate errors at the call site: `.context("sending request")`, `.context("parsing item")`
- Use `.with_context(|| format!(...))` when context string needs formatting: `.with_context(|| format!("reading config at {}", path.display()))`
- Use `anyhow::bail!(...)` for early-exit error conditions: `anyhow::bail!("--keep must be one of the two provided keys")`
- HTTP errors: check `resp.status_code >= 400` and bail with status + body
- 412 Precondition Failed handled specially in `patch_json` with a descriptive retry message
- Top-level `main()` delegates to `run() -> Result<()>`; errors printed via `eprintln!("{} {:#}", "error:".red().bold(), e)` then `process::exit(1)`
- No `unwrap()` in production paths; `.unwrap()` only in test helpers and `serde_json::to_value` calls where the input is a known-good struct

## Comments

**Style: codedoc block comments** (`/* */`) for module-level and function-group documentation; inline `//` only for short single-line notes.

**Module-level doc blocks** at the top of files explain design rationale:
```rust
/* ZoteroClient wraps the Zotero local connector API (localhost:23119/api).
Uses a synchronous HTTP client (minreq) -- each CLI invocation makes exactly
one request to localhost so async provides no benefit and only adds runtime
cold-start overhead. minreq without TLS keeps the dependency tree minimal. */
```

**Section separators** used in `client.rs` and `output.rs` to group related methods:
```rust
/* ------------------------------------------------------------------ */
/*  Core search / retrieval                                             */
/* ------------------------------------------------------------------ */
```

**Inline `//` comments** used only for short annotations:
```rust
// env var overrides
// Strip basic HTML tags for display
```

**Clap doc comments** (`///`) used for CLI argument/subcommand help strings in `main.rs`:
```rust
/// Key of the first item
key1: String,
/// Preview changes without applying them
#[arg(long)]
dry_run: bool,
```

## Function Design

**Size:** Functions are small and single-purpose. HTTP primitives (`get_json`, `post_json`, `patch_json`) are extracted as private helpers; public methods compose them.

**Parameters:** Prefer `&str` for string inputs, `&[T]` for slice inputs. Config passed by reference to constructors.

**Return Values:** `Result<T>` for fallible operations. Display functions return `String` (not `&str`) so callers decide whether to print or buffer.

## Module Design

**Exports:**
- All public items are `pub`; internal helpers are private (no `pub(crate)` used)
- `output.rs` helper functions (`truncate`, `wrap`, `strip_html`) are private; only table-builder functions are public
- `client.rs` HTTP primitives (`get_json`, `post_json`, `patch_json`, `lib_path`) are private

**Barrel Files:**
- Not used; `main.rs` declares modules with `mod` and imports selectively with `use`

## Serialization Conventions

- All API-facing structs derive `Deserialize, Serialize, Clone, Debug`
- `#[serde(rename_all = "camelCase")]` on `ItemData` to match Zotero API field names
- `#[serde(default)]` on `Vec` fields to handle missing arrays gracefully
- `#[serde(flatten)]` on `extra: serde_json::Map<...>` to capture unknown fields without data loss
- `#[serde(rename = "...")]` for individual field mismatches: `creator_type` <-> `"creatorType"`
- `CompactItem` is serialize-only (no `Deserialize`); `#[serde(rename = "type")]` for the `item_type` field

---

*Convention analysis: 2026-04-16*
