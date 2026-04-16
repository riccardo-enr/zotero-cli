# Testing Patterns

**Analysis Date:** 2026-04-16

## Test Framework

**Runner:**
- Rust built-in `cargo test` (no external test runner)
- No `jest.config`, `vitest.config`, or separate test framework
- Config: none required beyond `Cargo.toml`

**Assertion Library:**
- Rust standard `assert_eq!`, `assert!`, `assert!(result.is_err())`

**Run Commands:**
```bash
cargo test              # Run all tests
cargo test -- --nocapture   # Run with stdout visible
cargo clippy -- -D warnings  # Lint (CI enforced)
cargo build --release   # Verify release build
```

## Test File Organization

**Location:**
- Co-located: all tests live in `#[cfg(test)] mod tests { ... }` blocks at the bottom of each source file
- No separate `tests/` directory and no integration test files

**Files with tests:**
- `src/types.rs` -- 8 tests
- `src/config.rs` -- 7 tests
- `src/merge.rs` -- 8 tests
- `src/output.rs` -- 19 tests
- `src/client.rs` -- 0 tests (network layer; no mocking in place)

**Naming:**
- Test functions use descriptive `snake_case` names that read as sentences: `display_name_first_and_last`, `prefer_nonempty_target`, `structural_fields_unchanged`, `truncate_long_string_adds_ellipsis`

**Structure:**
```
src/
  client.rs        # No tests (requires live Zotero API)
  config.rs        # 7 unit tests
  main.rs          # No tests (CLI dispatch layer)
  merge.rs         # 8 unit tests
  output.rs        # 19 unit tests
  types.rs         # 8 unit tests
```

## Test Structure

**Suite Organization:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    /* ---- GroupName ---- */

    #[test]
    fn test_name() {
        // arrange
        // act
        // assert
    }
}
```

Tests within a module are grouped by comment headers: `/* ---- Creator::display_name ---- */`, `/* ---- CompactItem::from_item ---- */`, `/* ---- serde deserialization ---- */`.

**Patterns:**
- Arrange-Act-Assert (implicit, no framework-imposed structure)
- No `setup`/`teardown` hooks -- each test is self-contained
- Test helpers defined as private `fn make_item(...)` within the `mod tests` block
- `serde_json::json!(...)` macro used inline to construct JSON fixtures

## Mocking

**Framework:** None. No mock library (`mockall`, `mockito`, etc.) is used.

**Approach:**
- `ZoteroClient` is not tested with mock HTTP; network layer has zero test coverage
- Pure functions and serialization logic are tested directly
- Config tests use `toml::from_str` on inline strings rather than touching the filesystem

**What IS tested:**
- Serde deserialization/serialization roundtrips
- Pure transformation functions (`reconcile_items`, `build_dry_run_report`, `CompactItem::from_item`)
- Display/rendering helpers (`truncate`, `wrap`, `strip_html`, all table builders)
- `Creator::display_name` name formatting branches

**What is NOT tested (no mock infrastructure):**
- `ZoteroClient` HTTP methods (`get_json`, `post_json`, `patch_json`)
- All `client.rs` public methods (`search`, `get`, `children`, `collections`, etc.)
- `Config::load()` with env var overrides (env mutation is avoided)
- `main.rs` command dispatch

## Fixtures and Factories

**Test Data:**
Each test module defines a local `make_item` helper that constructs a minimal valid `ZoteroItem`:

```rust
// merge.rs
fn make_item(key: &str) -> ZoteroItem {
    ZoteroItem {
        key: key.into(),
        version: 1,
        data: ItemData {
            key: key.into(),
            version: Some(1),
            title: None,
            item_type: Some("journalArticle".into()),
            ...(all other fields zeroed/empty)
        },
    }
}

// output.rs
fn make_item(key: &str, title: &str, item_type: &str) -> ZoteroItem { ... }
```

JSON fixtures are constructed inline with `serde_json::json!(...)`:
```rust
let children = vec![serde_json::json!({
    "key": "A1",
    "data": {
        "itemType": "annotation",
        "annotationType": "highlight",
        "annotationText": "important text"
    }
})];
```

**Location:** Helpers are private functions inside `mod tests { }` in the same file as the code under test. No shared fixture files.

## Coverage

**Requirements:** None enforced -- no `cargo-tarpaulin` or `llvm-cov` configuration present.

**Untested surface area:**
- All of `src/client.rs` (HTTP calls)
- `Config::load()` env var branch
- `Config::save()`
- `main.rs` dispatch and error rendering
- `pluralise()` helper in `client.rs`

## CI Pipeline

**File:** `.github/workflows/ci.yml`

**Triggers:** push and pull_request to `main`

**Jobs (run in parallel):**

| Job | Command | Purpose |
|-----|---------|---------|
| `lint` | `cargo clippy -- -D warnings` | Clippy warnings as errors |
| `test` | `cargo test` | All unit tests |
| `build` | `cargo build --release` | Verify release compilation |

**Toolchain:** `dtolnay/rust-toolchain@stable` (latest stable Rust)

**Caching:** `Swatinem/rust-cache@v2` on all three jobs

**No coverage reporting** step in CI.

## Test Types

**Unit Tests:**
- All tests are unit tests of pure functions and serde logic
- Each test file covers its own module's private helpers and public API
- No external dependencies required to run

**Integration Tests:**
- None. No `tests/` directory. No end-to-end CLI invocation tests.

**E2E Tests:**
- Not used. The tool requires a live Zotero instance, which is not mocked.

## Common Patterns

**Testing serde deserialization:**
```rust
#[test]
fn item_data_deserializes_with_missing_optional_fields() {
    let json = r#"{"key": "ABC", "title": "Test"}"#;
    let data: ItemData = serde_json::from_str(json).unwrap();
    assert_eq!(data.key, "ABC");
    assert!(data.creators.is_empty());
}
```

**Testing error cases:**
```rust
#[test]
fn deserialize_invalid_toml_errors() {
    let toml = "not valid [[[ toml";
    let result: Result<Config, _> = toml::from_str(toml);
    assert!(result.is_err());
}
```

**Testing table output (smoke-test contains-check):**
```rust
#[test]
fn items_table_renders_rows() {
    let items = vec![make_item("ABC123", "Test Title", "journalArticle")];
    let result = items_table(&items);
    assert!(result.contains("ABC123"));
    assert!(result.contains("Test Title"));
}
```

**Testing merge semantics (property-based style):**
```rust
#[test]
fn prefer_nonempty_target() {
    let mut target = make_item("T1");
    target.data.title = Some("Target Title".into());
    let mut source = make_item("S1");
    source.data.title = Some("Source Title".into());
    let merged = reconcile_items(&target, &source);
    assert_eq!(merged["title"], "Target Title");
}
```

---

*Testing analysis: 2026-04-16*
