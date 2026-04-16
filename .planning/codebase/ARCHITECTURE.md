# Architecture

**Analysis Date:** 2026-04-16

## Pattern Overview

**Overall:** Layered CLI with thin command dispatch, synchronous HTTP client, and separated rendering.

**Key Characteristics:**
- Single binary, no async runtime (minreq over localhost; async overhead not justified)
- Read-only operations return typed structs; mutation operations use raw `serde_json::Value` for PATCH payloads
- Output format (table vs JSON) selected at the dispatch layer in `main.rs`; rendering modules know nothing about CLI flags
- Merge logic is pure (no I/O); all API calls for merge are made in `main.rs`, keeping `merge.rs` unit-testable

## Layers

**CLI / Dispatch (`src/main.rs`):**
- Purpose: Parse args (clap), load config, construct `ZoteroClient`, dispatch to subcommand handlers, select output format
- Location: `src/main.rs`
- Contains: `Cli`, `Commands`, `AddKind` (clap structs); `run()` function with full match on `Commands`
- Depends on: `client`, `config`, `types`, `output`, `merge`
- Used by: Binary entry point only

**HTTP Client (`src/client.rs`):**
- Purpose: Wrap all Zotero local connector API calls; build URLs; handle HTTP errors; deserialize responses
- Location: `src/client.rs`
- Contains: `ZoteroClient` struct; `get_json`, `post_json`, `patch_json` (private HTTP primitives); public methods per API operation
- Depends on: `config::Config`, `types::{ZoteroItem, ZoteroCollection}`, `minreq`, `serde_json`
- Used by: `main.rs`

**Configuration (`src/config.rs`):**
- Purpose: Load TOML config from `~/.config/zotero-cli/config.toml`; apply env var overrides; provide defaults
- Location: `src/config.rs`
- Contains: `Config` struct; `load()`, `save()`, `path()` methods
- Depends on: `dirs`, `toml`, `serde`
- Used by: `main.rs` (load), `client.rs` (read fields)

**Domain Types (`src/types.rs`):**
- Purpose: Typed representations of Zotero API response shapes; `CompactItem` for reduced-token LLM output
- Location: `src/types.rs`
- Contains: `ZoteroItem`, `ItemData`, `Creator`, `Tag`, `ZoteroCollection`, `CollectionData`, `CompactItem`
- Depends on: `serde`, `serde_json`
- Used by: `client.rs` (deserialization targets), `output.rs` (rendering), `merge.rs` (reconcile inputs), `main.rs`

**Output / Rendering (`src/output.rs`):**
- Purpose: Format data as human-readable terminal tables using `tabled`; strip HTML from notes; truncate/wrap text
- Location: `src/output.rs`
- Contains: `items_table`, `item_detail`, `annotations_table`, `notes_table`, `collections_table`, `tags_table`; private helpers `truncate`, `wrap`, `strip_html`
- Depends on: `types::{ZoteroItem, ZoteroCollection}`, `tabled`, `colored`
- Used by: `main.rs`

**Merge Logic (`src/merge.rs`):**
- Purpose: Pure reconciliation of two `ZoteroItem` values into a PATCH payload; dry-run report generation
- Location: `src/merge.rs`
- Contains: `reconcile_items`, `build_dry_run_report`; private helpers `is_empty`, `display_val`; `STRUCTURAL_FIELDS` constant
- Depends on: `types::ZoteroItem`, `serde_json`
- Used by: `main.rs` (merge subcommand handler)

## Data Flow

**Read command (e.g., search):**

1. `main.rs`: clap parses args into `Commands::Search { query, limit }`
2. `main.rs`: calls `client.search(&query, limit)` -> `Vec<ZoteroItem>`
3. `client.rs`: builds URL, calls `get_json`, deserializes JSON to `Vec<ZoteroItem>`
4. `main.rs`: if `--json`: serialize to JSON (compact via `CompactItem` by default); else call `output::items_table`
5. Print to stdout

**Merge command (multi-step mutation):**

1. `main.rs`: resolve target/source keys from `--keep` flag or positional order
2. `main.rs`: `client.get(target_key)` + `client.get(source_key)` -> two `ZoteroItem`s
3. `main.rs`: reject if either is attachment/note/annotation
4. `merge::reconcile_items(&target, &source)` -> merged `serde_json::Value` (pure, no I/O)
5. `client.children(source_key)` -> `Vec<Value>`
6. If `--dry-run`: print `merge::build_dry_run_report(...)`, return
7. `client.patch_item(target_key, ...)` with merged data
8. For each source child: `client.patch_item(child_key, ..., reparent_payload)`
9. Re-fetch source (version may have changed), `client.trash_item(source_key, ...)`
10. Print status to stderr

**State Management:**
- No in-process state between commands; each invocation is stateless
- Optimistic concurrency via `If-Unmodified-Since-Version` header on PATCH; 412 response produces a descriptive error

## Key Abstractions

**`ZoteroItem`:**
- Purpose: Typed wrapper for a Zotero library item; `data` field holds `ItemData` with known fields plus `#[serde(flatten)] extra` map for unknown fields
- Examples: `src/types.rs` lines 57-61
- Pattern: Newtype wrapper `{ key, version, data }` mirrors the Zotero API envelope

**`CompactItem`:**
- Purpose: Reduced representation for LLM-friendly JSON output; strips abstract, url, doi, tags; filters creators to authors only
- Examples: `src/types.rs` lines 65-92
- Pattern: Projection type with a factory `from_item(&ZoteroItem) -> Self`

**`ZoteroClient`:**
- Purpose: Single access point for all HTTP operations; holds connection parameters; provides typed public methods per endpoint
- Examples: `src/client.rs`
- Pattern: Facade over three HTTP primitives (`get_json`, `post_json`, `patch_json`)

**`Config`:**
- Purpose: Layered configuration (file -> env vars -> CLI flag); sensible defaults for localhost operation
- Examples: `src/config.rs`
- Pattern: `Default` impl + `load()` that applies overrides; env vars `ZOTERO_API_BASE`, `ZOTERO_API_KEY`; `--api-base` CLI flag

**`STRUCTURAL_FIELDS` constant:**
- Purpose: Defines which `ItemData` fields are identity/system fields and must not be overwritten during merge
- Examples: `src/merge.rs` lines 9-18
- Pattern: Static slice used as an exclusion filter in `reconcile_items`

## Entry Points

**Binary entry point:**
- Location: `src/main.rs` (`fn main`)
- Triggers: `zotero-cli <subcommand> [args]`
- Responsibilities: Error formatting (`anyhow` chain); non-zero exit on failure

**`run() -> Result<()>`:**
- Location: `src/main.rs` line 118
- Triggers: Called by `main`
- Responsibilities: Arg parsing, config loading, client construction, full command dispatch

## Error Handling

**Strategy:** `anyhow::Result` propagated from all layers; printed as `error: <chain>` at top level with `{:#}` formatting for full cause chain.

**Patterns:**
- HTTP 4xx/5xx: `anyhow::bail!` with status code and body
- HTTP 412: specific "version conflict -- retry" message
- Deserialization: `.context("parsing ...")` attached to `serde_json::from_str`
- Invalid args: `anyhow::bail!` before any I/O (e.g., `--keep` not matching either key; merging attachment types)

## Cross-Cutting Concerns

**Logging:** None; status messages printed to stderr (`eprintln!`) after successful mutations; errors to stderr via `main`.
**Validation:** Input validation in `main.rs` dispatch (item type guard, `--keep` key check); API-level validation deferred to Zotero.
**Authentication:** Optional `Zotero-API-Key` header injected by `ZoteroClient` HTTP primitives when `api_key` is set; works without key against the local connector default.

---

*Architecture analysis: 2026-04-16*
