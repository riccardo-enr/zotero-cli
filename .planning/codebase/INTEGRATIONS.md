# External Integrations

**Analysis Date:** 2026-04-16

## APIs & External Services

**Zotero Local Connector API:**
- Base URL: `http://localhost:23119/api` (default; overridable via config or `--api-base`)
- Protocol: HTTP/1.1, JSON request/response bodies
- API version: `3` (sent as `?v=3` query param on every request)
- Auth: optional `Zotero-API-Key` header (not required for local connector in default setup)
- Client implementation: `src/client.rs` (`ZoteroClient`)
- All requests are synchronous (minreq); 30-second timeout per request
- Optimistic concurrency: PATCH requests send `If-Unmodified-Since-Version` header; 412 response triggers a user-facing retry message

**Zotero Web Translation Server (optional):**
- URL: `http://localhost:1969/web` (hardcoded constant `TRANSLATOR_URL` in `src/client.rs`)
- Used only by `add url` subcommand (`ZoteroClient::add_url`)
- Payload: `{"url": "<url>", "sessionID": "zotero-cli"}`
- No auth header sent to this endpoint

## Endpoint Inventory

All endpoints are called from `src/client.rs`. The library-scoped path prefix is built by `lib_path()` as `/{users|groups}/{id}`.

| Method | Path pattern | Used by |
|--------|-------------|---------|
| GET | `{base}/{lib}/items?q=...&limit=...&v=3` | `search` |
| GET | `{base}/{lib}/items/{key}?v=3` | `get`, `merge` |
| GET | `{base}/{lib}/items/{key}/children?v=3` | `annotations`, `notes`, `merge` |
| GET | `{base}/{lib}/collections?v=3` | `collections` |
| GET | `{base}/{lib}/collections/{id}/items?v=3` | `collection` |
| GET | `{base}/{lib}/tags?v=3` | `tags` |
| GET | `{base}/{lib}/items?sort=dateAdded&direction=desc&limit=N&v=3` | `recent` |
| POST | `{base}/items?v=3` | `add doi` |
| POST | `http://localhost:1969/web` | `add url` |
| PATCH | `{base}/{lib}/items/{key}?v=3` | `merge` (update metadata, re-parent children, trash source) |

## Data Storage

**Databases:**
- None managed by this CLI. All data lives inside the running Zotero application.

**File Storage:**
- Config file at `~/.config/zotero-cli/config.toml` (read/write, `src/config.rs`)
- No other files written at runtime

**Caching:**
- None. Each CLI invocation issues fresh requests.

## Authentication & Identity

**Auth Provider:**
- Zotero API key (optional for local connector, required for remote API access)
- Set via `ZOTERO_API_KEY` env var or `api_key` field in config file
- Sent as `Zotero-API-Key` HTTP header on every request to the connector API
- Not sent to the translation server (`localhost:1969`)

**Library scoping:**
- `library_type`: `"user"` (personal library) or `"group"` (group library)
- `user_id`: numeric ID; `0` resolves to the currently authenticated local user

## Data Formats

**Input (from API):**
- JSON arrays of `ZoteroItem` objects (`src/types.rs`: `ZoteroItem`, `ItemData`, `Creator`, `Tag`)
- Item data uses `camelCase` field names (handled via `#[serde(rename_all = "camelCase")]`)
- Unknown/extra fields captured losslessly via `#[serde(flatten)] extra: serde_json::Map<String, Value>` in `ItemData`

**Output (to stdout):**
- Human-readable: ANSI-colored tables via `tabled` with `Style::modern()` (`src/output.rs`)
- JSON: `serde_json::to_string_pretty(...)` — full payload or compact (`CompactItem` in `src/types.rs`)
- Compact JSON strips: `abstract_note`, `url`, `doi`, `tags` — designed for LLM piping
- Error messages: written to stderr with `colored` formatting (`src/main.rs`)

## Monitoring & Observability

**Error Tracking:**
- None. Errors surface as stderr messages and non-zero exit codes.

**Logs:**
- None. Diagnostic output only on error or after mutating operations (merge completion).

## CI/CD & Deployment

**Hosting:**
- Distributed as a compiled binary (no package registry publishing configured)

**CI Pipeline:**
- GitHub Actions: `.github/workflows/ci.yml`
- Triggers: push/PR to `main`
- Jobs: `lint` (clippy -D warnings), `test` (cargo test), `build` (cargo build --release)
- Runner: `ubuntu-latest`; Rust cache via `Swatinem/rust-cache@v2`

## Environment Configuration

**Required env vars:**
- None strictly required (all have defaults for local use)

**Optional env vars:**
- `ZOTERO_API_BASE` - override API base URL (e.g. for remote Zotero instance)
- `ZOTERO_API_KEY` - Zotero API authentication key

**Secrets location:**
- API key stored in `~/.config/zotero-cli/config.toml` on disk, or in shell environment

## Webhooks & Callbacks

**Incoming:**
- None. CLI is purely request-driven.

**Outgoing:**
- None beyond the Zotero API calls described above.

---

*Integration audit: 2026-04-16*
