# zotero-cli

Terminal interface for [Zotero](https://www.zotero.org/), mirroring the MCP operations without needing an LLM. Talks to the Zotero local connector API on `localhost:23119`.

## Prerequisites

Zotero must be running. Verify with:

```sh
curl -s http://localhost:23119/connector/ping
# → <!DOCTYPE html><html><body>Zotero is running</body></html>
```

## Install

```sh
cargo install --path .
# installs zotero-cli to ~/.cargo/bin
```

Or just build without installing:

```sh
cargo build --release
# binary at: target/release/zotero-cli
```

## Quick check

```sh
zotero-cli --help
zotero-cli recent 5          # last 5 added items
zotero-cli search "author year"
zotero-cli config            # show current config path and values
```

## Usage

```
zotero-cli [OPTIONS] <COMMAND>

Options:
  --json        Output raw JSON (works on all subcommands)
  --api-base    Override API base URL

Commands:
  search <query> [-l <limit>]   Search items by keyword (default limit: 25)
  get <key>                     Full metadata for an item
  annotations <key>             PDF annotations attached to an item
  notes <key>                   Notes attached to an item
  collections                   List all collections
  collection <id>               List items in a collection
  add doi <doi>                 Add item by DOI
  add url <url>                 Add item by URL
  tags                          List all tags
  recent [n]                    N most recently added items (default: 10)
  config                        Show config file path and active settings
```

## Configuration

Config file: `~/.config/zotero-cli/config.toml`

```toml
api_base     = "http://localhost:23119/api"   # default
api_key      = ""                             # optional, for web API
user_id      = 0                              # optional, for web API
library_type = "user"                         # "user" or "group"
```

Without `user_id` set, all requests go to the local Zotero instance (no API key needed).

## JSON output

Every subcommand supports `--json` for piping:

```sh
zotero-cli search "Agha-mohammadi" --json | jq '.[].data.title'
zotero-cli recent 20 --json | jq '.[].data.key'
```

## Use as a drop-in for the Zotero MCP in Claude Code

`zotero-cli` can replace `mcp__zotero__*` tool calls inside Claude Code sessions.
Run it with the `!` prefix to pipe output directly into the conversation:

```sh
! zotero-cli search "mppi" --json | jq '[.[] | select(.data.itemType != "attachment")]'
! zotero-cli get PEPC47XF --json
! zotero-cli recent 10 --json
```

### Why bother?

- Works outside Claude Code (shell scripts, other editors, cron jobs)
- No MCP server process required
- Filter and reshape the JSON with `jq` before it hits the context window

### Benchmark (measured on Linux, Zotero 8 local API)

| Operation | Latency | Raw payload | After `jq` strip |
|---|---|---|---|
| `search "mppi"` (25 results) | ~78 ms | ~11 k tokens | ~4 k tokens |
| `recent 10` | ~76 ms | ~2.9 k tokens | ~1 k tokens |

Token estimates use the 1 token ≈ 4 chars rule. "After `jq` strip" removes
attachment items and keeps only `key`, `title`, `itemType`, `date`, `creators`,
`tags`, and `doi`.

Recommended `jq` filter for Claude Code use:

```sh
zotero-cli search "query" --json | jq '[
  .[] | select(.data.itemType != "attachment") | {
    key: .key,
    title: .data.title,
    authors: [.data.creators[] | select(.creatorType=="author") | .lastName],
    date: .data.date,
    doi: .data.doi,
    tags: [.data.tags[].tag]
  }
]'
```

## Static binary (TODO)

Build a fully static binary with [`cross`](https://github.com/cross-rs/cross):

```sh
cross build --target x86_64-unknown-linux-musl --release
```
