# Codebase Concerns

**Analysis Date:** 2026-04-16

## Tech Debt

**`unwrap()` calls in production merge path:**
- Issue: `src/merge.rs` lines 28-32 and 70 use `.unwrap()` on `serde_json::to_value` and `.as_object_mut()` inside `reconcile_items`. These are called on `ZoteroItem` structs that always serialize cleanly, so they are practically safe, but any future field change that breaks `Serialize` would panic instead of returning a `Result`.
- Files: `src/merge.rs` (lines 28, 29, 31, 32, 70, 107, 108, 109)
- Impact: Silent panic instead of a clean error message during merge operations.
- Fix approach: Replace with `serde_json::to_value(...).context(...)` chains and propagate `Result` from `reconcile_items`.

**`add_doi` hardcodes `itemType: journalArticle`:**
- Issue: `src/client.rs` lines 206-209 submit a manually constructed JSON payload with `itemType: journalArticle` for every DOI add. This is wrong for DOIs belonging to books, preprints, conference papers, etc. The Zotero connector would normally resolve the type from the DOI.
- Files: `src/client.rs` (lines 204-212)
- Impact: Added items always have incorrect type when the DOI is not a journal article.
- Fix approach: POST the DOI to the translator endpoint at `http://localhost:1969/web` instead of building a stub payload, or use a dedicated `/doi` endpoint if the local connector exposes one.

**`Config::save` is dead code:**
- Issue: `src/config.rs` line 62 has `#[allow(dead_code)]` on `Config::save`. No command writes config back.
- Files: `src/config.rs` (lines 61-70)
- Impact: No functional breakage; dead code increases maintenance surface.
- Fix approach: Either implement a `zotero-cli config set <key> <value>` subcommand that uses it, or remove the method.

**`library_type` validation is absent:**
- Issue: `src/config.rs` accepts any string for `library_type`. `src/client.rs` `pluralise()` only handles `"user"` and `"group"` — any other value is passed through as-is, generating a malformed URL path.
- Files: `src/config.rs` (line 18), `src/client.rs` (lines 236-242)
- Impact: Silent wrong URL if a user sets `library_type = "personal"` or similar typo; results in a confusing 404/400 from the API.
- Fix approach: Validate in `Config::load` with an enum (`LibraryType`) or an explicit allowlist check, returning an error early.

**Pagination is not implemented:**
- Issue: All list endpoints (`search`, `recent`, `collection_items`, `collections`, `tags`) make a single request. The Zotero API paginates at 100 items by default. Large libraries silently return a partial result with no warning.
- Files: `src/client.rs` (lines 81-153)
- Impact: `zotero-cli collections` or `zotero-cli tags` silently truncates large libraries.
- Fix approach: Loop with `start` offset until the response is smaller than the page size, or expose `--limit` on affected commands and document the truncation.

**`sessionID` in `add_url` is hardcoded:**
- Issue: `src/client.rs` line 216 sends `"sessionID": "zotero-cli"` as a constant. If multiple concurrent CLI invocations run, they share the same translator session and may interfere.
- Files: `src/client.rs` (line 216)
- Impact: Rare in practice (CLI is single-shot), but a correctness issue if the tool is ever wrapped in a loop or script.
- Fix approach: Generate a random UUID per invocation using `uuid` crate, or document the limitation.

## Known Bugs

**Merge does not re-fetch source version after each child reparent:**
- Symptoms: During merge, each child PATCH changes the source item's version in Zotero. The code re-fetches source version once after all children are moved (line 318), but children are patched sequentially without re-fetching intermediate versions. If a child PATCH fails mid-way, the subsequent `trash_item` call uses a stale version and gets a 412.
- Files: `src/main.rs` (lines 305-319)
- Trigger: Merge with 2+ children where any child reparent fails.
- Workaround: Retry the merge; the target already has the patched data so only children and trash need to complete.

**`display_val` truncation may cut in the middle of a multi-byte UTF-8 sequence:**
- Symptoms: `src/merge.rs` `display_val` (lines 172-175) slices `&s[..57]` on a `String` using byte indices. This can panic or produce garbled output if the string contains non-ASCII characters near the 57-byte boundary.
- Files: `src/merge.rs` (lines 172, 183)
- Trigger: Dry-run report on items with non-ASCII titles or field values longer than 60 chars.
- Workaround: None; will panic at runtime.
- Fix approach: Use `s.chars().take(57).collect::<String>()` consistent with the `truncate` helper in `src/output.rs`.

## Security Considerations

**API key transmitted over plain HTTP:**
- Risk: The default `api_base` is `http://localhost:23119/api` (plain HTTP). If `api_base` is overridden to a remote host (which the `--api-base` flag and `ZOTERO_API_BASE` env var both allow), the API key in the `Zotero-API-Key` header travels unencrypted.
- Files: `src/client.rs` (lines 33-47), `src/config.rs` (line 22)
- Current mitigation: Default is localhost only; `minreq` has no TLS feature enabled.
- Recommendations: Document that remote `api_base` values must use HTTPS; add a compile-time warning or runtime check when an `http://` non-localhost URL is used with an API key present. If remote access becomes a supported use case, re-enable TLS in `minreq`.

**API key visible in process environment:**
- Risk: `ZOTERO_API_KEY` env var is read at startup and stored in `Config`. On Linux, `/proc/<pid>/environ` exposes env vars to the same user. This is standard practice but worth documenting.
- Files: `src/config.rs` (lines 55-57)
- Current mitigation: Key is only used in-process; not logged or printed (the `Config` subcommand truncates it).
- Recommendations: No action needed for localhost use; acceptable risk.

## Performance Bottlenecks

**Merge makes N+3 sequential HTTP requests:**
- Problem: For a merge with N children: 2 GETs (target, source) + 1 PATCH (target data) + N PATCHs (children reparent) + 1 GET (re-fetch source version) + 1 PATCH (trash). All are sequential. With large child counts (e.g. a paper with 50 annotations), this is slow.
- Files: `src/main.rs` (lines 274-319)
- Cause: Zotero local connector does not expose a batch write endpoint; sequential is the only option per-item. The re-fetch (line 318) adds an extra round-trip unnecessarily — the version could be tracked from patch response headers.
- Improvement path: Parse the `Last-Modified-Version` response header from each PATCH to track the latest version without a separate GET.

## Fragile Areas

**`serde(flatten)` + `serde_json::Map` interaction in merge:**
- Files: `src/types.rs` (line 26), `src/merge.rs` (lines 28-42)
- Why fragile: `ItemData` uses `#[serde(flatten)]` to capture unknown fields in `extra`. When serialized to a `serde_json::Value` for merging, the flattened keys appear at the top level of the object. The merge loop in `reconcile_items` then iterates over ALL top-level keys including these flattened extras. This is correct but non-obvious; adding a new named field to `ItemData` in the future may silently duplicate it (once via the named field, once via `extra` if it was previously captured there from an old response).
- Safe modification: After adding any new named field to `ItemData`, verify that `extra` no longer captures it in real API responses before merging.
- Test coverage: No test covers the flattened-extra round-trip through `reconcile_items`.

**`parentCollection` field is `Option<serde_json::Value>`:**
- Files: `src/types.rs` (lines 99-101), `src/output.rs` (lines 178-183)
- Why fragile: The Zotero API returns `parentCollection: false` (boolean) for root collections, not `null` or the field's absence. The current code handles this with a special-case check in `collections_table`. Any other consumer of `CollectionData.parent_collection` must repeat this same special-case logic or silently treat `false` as a valid key string.
- Safe modification: Add a helper method `CollectionData::parent_key() -> Option<&str>` that encapsulates the `false`/`null`/string disambiguation.
- Test coverage: The `false` case is handled in output but not unit-tested.

## Test Coverage Gaps

**No integration tests against a real or mocked HTTP server:**
- What's not tested: All of `src/client.rs` — `get_json`, `post_json`, `patch_json`, HTTP error handling (400, 412, 5xx), timeout behavior, URL construction with special characters in query strings.
- Files: `src/client.rs` (all 243 lines)
- Risk: A URL construction bug (e.g. missing `v=` parameter, double-encoding) would only surface at runtime.
- Priority: High

**No test for merge failure recovery (partial state):**
- What's not tested: Behavior when a child reparent PATCH fails mid-merge. The target has already been patched; the source has not been trashed. The library is left in a partially merged state.
- Files: `src/main.rs` (lines 305-319)
- Risk: Silent data inconsistency after a network hiccup or version conflict during merge.
- Priority: High

**`add_doi` and `add_url` have zero tests:**
- What's not tested: Payload construction, translator endpoint URL, response parsing.
- Files: `src/client.rs` (lines 204-233)
- Risk: Regressions in add workflows are invisible until manual testing.
- Priority: Medium

**`Config::load` env-var override path not tested:**
- What's not tested: The `ZOTERO_API_BASE` and `ZOTERO_API_KEY` env var override logic in `Config::load`.
- Files: `src/config.rs` (lines 52-58)
- Risk: A refactor could silently break env var support.
- Priority: Low

---

*Concerns audit: 2026-04-16*
