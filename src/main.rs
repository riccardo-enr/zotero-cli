mod client;
mod config;
mod merge;
mod output;
mod types;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use colored::Colorize;

use client::ZoteroClient;
use config::Config;
use types::CompactItem;

/* Zotero CLI — terminal interface for Zotero, mirroring the MCP operations.
Talks to the Zotero local connector API running at localhost:23119.
All subcommands default to human-readable table output; pass --json
to emit raw JSON suitable for piping into jq or other tools.
Combine --json with --compact to strip verbose fields (abstract, url, doi,
tags) for lower-token output when piping to an LLM. */

#[derive(Parser)]
#[command(
    name = "zotero-cli",
    about = "Terminal interface for your Zotero library",
    version,
    propagate_version = true
)]
struct Cli {
    /* Emit raw JSON instead of human-readable tables */
    #[arg(long, global = true, help = "Output raw JSON")]
    json: bool,

    /* Include verbose fields (abstract, url, doi, tags) in JSON list output.
       By default list commands emit compact JSON; pass --no-compact to get
       the full payload. */
    #[arg(long, global = true, help = "Include verbose fields in JSON output (abstract, url, doi, tags)")]
    no_compact: bool,

    /* Override the API base URL (useful for debugging or remote instances) */
    #[arg(
        long,
        global = true,
        value_name = "URL",
        help = "Override API base URL"
    )]
    api_base: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Search items by keyword")]
    Search {
        query: String,
        #[arg(short, long, default_value_t = 25, help = "Max results")]
        limit: usize,
    },
    #[command(about = "Get full metadata for an item")]
    Get { key: String },
    #[command(about = "List PDF annotations for an item")]
    Annotations { key: String },
    #[command(about = "List notes attached to an item")]
    Notes { key: String },
    #[command(about = "List all collections")]
    Collections,
    #[command(about = "List items in a collection")]
    Collection { id: String },
    #[command(about = "Add an item by DOI or URL")]
    Add {
        #[command(subcommand)]
        kind: AddKind,
    },
    #[command(about = "List all tags in the library")]
    Tags,
    #[command(about = "Show the N most recently added items (default: 10)")]
    Recent {
        #[arg(default_value_t = 10)]
        n: usize,
    },
    #[command(about = "Merge two items (combine metadata, children, tags, collections)")]
    Merge {
        /// Key of the first item
        key1: String,
        /// Key of the second item
        key2: String,
        /// Preview changes without applying them
        #[arg(long, help = "Preview changes without applying them")]
        dry_run: bool,
        /// Key of the item to keep as primary (default: first key)
        #[arg(long, value_name = "KEY", help = "Key of the item to keep as primary")]
        keep: Option<String>,
    },
    #[command(about = "Print config file path and active settings")]
    Config,
    #[command(about = "Generate shell completions")]
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
enum AddKind {
    Doi { doi: String },
    Url { url: String },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{} {:#}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let mut cfg = Config::load()?;
    if let Some(base) = cli.api_base {
        cfg.api_base = base;
    }

    let client = ZoteroClient::new(&cfg)?;

    match cli.command {
        Commands::Search { query, limit } => {
            let items = client.search(&query, limit)?;
            if cli.json {
                if !cli.no_compact {
                    let compact: Vec<CompactItem> =
                        items.iter().map(CompactItem::from_item).collect();
                    println!("{}", serde_json::to_string_pretty(&compact)?);
                } else {
                    println!("{}", serde_json::to_string_pretty(&items)?);
                }
            } else {
                println!("{}", output::items_table(&items));
            }
        }

        Commands::Get { key } => {
            let item = client.get(&key)?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&item)?);
            } else {
                println!("{}", output::item_detail(&item));
            }
        }

        Commands::Annotations { key } => {
            let children = client.children(&key)?;
            if cli.json {
                let annotations: Vec<&serde_json::Value> = children
                    .iter()
                    .filter(|c| {
                        c.get("data")
                            .and_then(|d| d.get("itemType"))
                            .and_then(|t| t.as_str())
                            == Some("annotation")
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&annotations)?);
            } else {
                println!("{}", output::annotations_table(&children));
            }
        }

        Commands::Notes { key } => {
            let children = client.children(&key)?;
            if cli.json {
                let notes: Vec<&serde_json::Value> = children
                    .iter()
                    .filter(|c| {
                        c.get("data")
                            .and_then(|d| d.get("itemType"))
                            .and_then(|t| t.as_str())
                            == Some("note")
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&notes)?);
            } else {
                println!("{}", output::notes_table(&children));
            }
        }

        Commands::Collections => {
            let cols = client.collections()?;
            if cli.json {
                if !cli.no_compact {
                    let compact: Vec<serde_json::Value> = cols
                        .iter()
                        .map(|c| serde_json::json!({"key": c.key, "name": c.data.name}))
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&compact)?);
                } else {
                    println!("{}", serde_json::to_string_pretty(&cols)?);
                }
            } else {
                println!("{}", output::collections_table(&cols));
            }
        }

        Commands::Collection { id } => {
            let items = client.collection_items(&id)?;
            if cli.json {
                if !cli.no_compact {
                    let compact: Vec<CompactItem> =
                        items.iter().map(CompactItem::from_item).collect();
                    println!("{}", serde_json::to_string_pretty(&compact)?);
                } else {
                    println!("{}", serde_json::to_string_pretty(&items)?);
                }
            } else {
                println!("{}", output::items_table(&items));
            }
        }

        Commands::Add { kind } => match kind {
            AddKind::Doi { doi } => {
                let result = client.add_doi(&doi)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            AddKind::Url { url } => {
                let result = client.add_url(&url)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        },

        Commands::Tags => {
            let tags = client.tags()?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&tags)?);
            } else {
                println!("{}", output::tags_table(&tags));
            }
        }

        Commands::Recent { n } => {
            let items = client.recent(n)?;
            if cli.json {
                if !cli.no_compact {
                    let compact: Vec<CompactItem> =
                        items.iter().map(CompactItem::from_item).collect();
                    println!("{}", serde_json::to_string_pretty(&compact)?);
                } else {
                    println!("{}", serde_json::to_string_pretty(&items)?);
                }
            } else {
                println!("{}", output::items_table(&items));
            }
        }

        Commands::Merge {
            key1,
            key2,
            dry_run,
            keep,
        } => {
            let (target_key, source_key) = if let Some(ref k) = keep {
                if k == &key1 {
                    (&key1, &key2)
                } else if k == &key2 {
                    (&key2, &key1)
                } else {
                    anyhow::bail!("--keep must be one of the two provided keys");
                }
            } else {
                (&key1, &key2)
            };

            let target = client.get(target_key)?;
            let source = client.get(source_key)?;

            /* Reject merge of non-top-level items */
            let reject_types = ["attachment", "note", "annotation"];
            for (label, item) in [("target", &target), ("source", &source)] {
                if let Some(ref t) = item.data.item_type {
                    if reject_types.contains(&t.as_str()) {
                        anyhow::bail!(
                            "{} ({}) is a {} -- only top-level items can be merged",
                            label,
                            item.key,
                            t
                        );
                    }
                }
            }

            let merged_data = merge::reconcile_items(&target, &source);
            let source_children = client.children(source_key)?;

            if dry_run {
                println!(
                    "{}",
                    merge::build_dry_run_report(&target, &source, &merged_data, &source_children)
                );
                return Ok(());
            }

            client.patch_item(target_key, target.version, &merged_data)?;

            for child in &source_children {
                let child_key = child["key"]
                    .as_str()
                    .context("child item missing key")?;
                let child_version = child["version"]
                    .as_u64()
                    .context("child item missing version")?;
                let reparent = serde_json::json!({"parentItem": target_key});
                client.patch_item(child_key, child_version, &reparent)?;
            }

            /* Re-fetch source to get its latest version (may have changed
               when children were re-parented). */
            let source_fresh = client.get(source_key)?;
            client.trash_item(source_key, source_fresh.version)?;

            eprintln!(
                "{} merged {} into {}",
                "done:".green().bold(),
                source_key,
                target_key
            );
            if !source_children.is_empty() {
                eprintln!("  {} child item(s) re-parented", source_children.len());
            }
            eprintln!("  {} moved to trash", source_key);
        }

        Commands::Completions { shell } => {
            clap_complete::generate(
                shell,
                &mut Cli::command(),
                "zotero-cli",
                &mut std::io::stdout(),
            );
        }

        Commands::Config => {
            println!("{}", Config::path().display());
            println!();
            println!("  api_base:     {}", cfg.api_base);
            println!(
                "  api_key:      {}",
                cfg.api_key
                    .as_deref()
                    .map(|k| format!("{}…", &k[..k.len().min(8)]))
                    .unwrap_or_else(|| "(not set)".to_string())
            );
            println!(
                "  user_id:      {}",
                cfg.user_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "(not set)".to_string())
            );
            println!("  library_type: {}", cfg.library_type);
        }
    }

    Ok(())
}
