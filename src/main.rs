mod client;
mod config;
mod output;
mod types;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

use client::ZoteroClient;
use config::Config;

/* Zotero CLI — terminal interface for Zotero, mirroring the MCP operations.
Talks to the Zotero local connector API running at localhost:23119.
All subcommands default to human-readable table output; pass --json
to emit raw JSON suitable for piping into jq or other tools. */

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
    /* Search items by keyword */
    Search {
        query: String,
        #[arg(short, long, default_value_t = 25)]
        limit: usize,
    },
    /* Get full metadata for an item by its key */
    Get {
        key: String,
    },
    /* List PDF annotations for an item */
    Annotations {
        key: String,
    },
    /* List notes attached to an item */
    Notes {
        key: String,
    },
    /* List all collections */
    Collections,
    /* List items in a collection */
    Collection {
        id: String,
    },
    /* Add an item by DOI or URL */
    Add {
        #[command(subcommand)]
        kind: AddKind,
    },
    /* List all tags in the library */
    Tags,
    /* Show the N most recently added items */
    Recent {
        #[arg(default_value_t = 10)]
        n: usize,
    },
    /* Print the config file path and current settings */
    Config,
}

#[derive(Subcommand)]
enum AddKind {
    Doi { doi: String },
    Url { url: String },
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{} {:#}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    let mut cfg = Config::load()?;
    if let Some(base) = cli.api_base {
        cfg.api_base = base;
    }

    let client = ZoteroClient::new(&cfg)?;

    match cli.command {
        Commands::Search { query, limit } => {
            let items = client.search(&query, limit).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&items)?);
            } else {
                println!("{}", output::items_table(&items));
            }
        }

        Commands::Get { key } => {
            let item = client.get(&key).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&item)?);
            } else {
                println!("{}", output::item_detail(&item));
            }
        }

        Commands::Annotations { key } => {
            let children = client.children(&key).await?;
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
            let children = client.children(&key).await?;
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
            let cols = client.collections().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&cols)?);
            } else {
                println!("{}", output::collections_table(&cols));
            }
        }

        Commands::Collection { id } => {
            let items = client.collection_items(&id).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&items)?);
            } else {
                println!("{}", output::items_table(&items));
            }
        }

        Commands::Add { kind } => match kind {
            AddKind::Doi { doi } => {
                let result = client.add_doi(&doi).await?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            AddKind::Url { url } => {
                let result = client.add_url(&url).await?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        },

        Commands::Tags => {
            let tags = client.tags().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&tags)?);
            } else {
                println!("{}", output::tags_table(&tags));
            }
        }

        Commands::Recent { n } => {
            let items = client.recent(n).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&items)?);
            } else {
                println!("{}", output::items_table(&items));
            }
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
