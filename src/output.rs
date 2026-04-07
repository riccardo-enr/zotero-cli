use colored::Colorize;
use tabled::{builder::Builder, settings::Style};

use crate::types::{ZoteroCollection, ZoteroItem};

const TITLE_MAX_WIDTH: usize = 55;
const AUTHORS_MAX_WIDTH: usize = 30;
const ANNOTATION_TEXT_MAX_WIDTH: usize = 50;
const ANNOTATION_COMMENT_MAX_WIDTH: usize = 40;
const NOTE_MAX_WIDTH: usize = 80;

/* Human-readable table renderers. Each function returns a String so callers
   can decide whether to print or buffer. */

pub fn items_table(items: &[ZoteroItem]) -> String {
    if items.is_empty() {
        return format!("{}", "No items found.".yellow());
    }
    let mut builder = Builder::default();
    builder.push_record(["Key", "Type", "Title", "Authors", "Date"]);
    for item in items {
        let d = &item.data;
        let title = truncate(d.title.as_deref().unwrap_or("—"), TITLE_MAX_WIDTH);
        let authors = d
            .creators
            .iter()
            .filter(|c| c.creator_type.as_deref() == Some("author"))
            .map(|c| c.display_name())
            .collect::<Vec<_>>()
            .join("; ");
        let authors = truncate(if authors.is_empty() { "—" } else { &authors }, AUTHORS_MAX_WIDTH);
        builder.push_record([
            &d.key,
            d.item_type.as_deref().unwrap_or("—"),
            &title,
            &authors,
            d.date.as_deref().unwrap_or("—"),
        ]);
    }
    builder.build()
        .with(Style::modern())
        .to_string()
}

pub fn item_detail(item: &ZoteroItem) -> String {
    let d = &item.data;
    let mut out = String::new();
    let title = d.title.as_deref().unwrap_or("(no title)");
    out.push_str(&format!("{}\n", title.bold()));
    out.push_str(&format!("  Key:      {}\n", d.key.cyan()));
    out.push_str(&format!(
        "  Type:     {}\n",
        d.item_type.as_deref().unwrap_or("—")
    ));
    out.push_str(&format!("  Date:     {}\n", d.date.as_deref().unwrap_or("—")));

    let authors: Vec<String> = d
        .creators
        .iter()
        .filter(|c| c.creator_type.as_deref() == Some("author"))
        .map(|c| c.display_name())
        .collect();
    if !authors.is_empty() {
        out.push_str(&format!("  Authors:  {}\n", authors.join("; ")));
    }

    if let Some(doi) = &d.doi {
        out.push_str(&format!("  DOI:      {}\n", doi.blue()));
    }
    if let Some(url) = &d.url {
        out.push_str(&format!("  URL:      {}\n", url.blue()));
    }

    if let Some(abs) = &d.abstract_note {
        out.push_str(&format!("\n{}\n{}\n", "Abstract:".bold(), wrap(abs, 80)));
    }

    if !d.tags.is_empty() {
        let tags: Vec<&str> = d.tags.iter().map(|t| t.tag.as_str()).collect();
        out.push_str(&format!("\n  Tags:  {}\n", tags.join(", ").green()));
    }
    out
}

pub fn annotations_table(children: &[serde_json::Value]) -> String {
    let annotations: Vec<&serde_json::Value> = children
        .iter()
        .filter(|c| {
            c.get("data")
                .and_then(|d| d.get("itemType"))
                .and_then(|t| t.as_str())
                == Some("annotation")
        })
        .collect();

    if annotations.is_empty() {
        return format!("{}", "No annotations found.".yellow());
    }

    let mut builder = Builder::default();
    builder.push_record(["Key", "Type", "Page", "Text", "Comment"]);
    for ann in annotations {
        let d = ann.get("data").unwrap_or(ann);
        let key = ann
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let atype = d
            .get("annotationType")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let page = d
            .get("annotationPageLabel")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let text = truncate(
            d.get("annotationText")
                .and_then(|v| v.as_str())
                .unwrap_or("—"),
            ANNOTATION_TEXT_MAX_WIDTH,
        );
        let comment = truncate(
            d.get("annotationComment")
                .and_then(|v| v.as_str())
                .unwrap_or("—"),
            ANNOTATION_COMMENT_MAX_WIDTH,
        );
        builder.push_record([key, atype, page, &text, &comment]);
    }
    builder.build()
        .with(Style::modern())
        .to_string()
}

pub fn notes_table(children: &[serde_json::Value]) -> String {
    let notes: Vec<&serde_json::Value> = children
        .iter()
        .filter(|c| {
            c.get("data")
                .and_then(|d| d.get("itemType"))
                .and_then(|t| t.as_str())
                == Some("note")
        })
        .collect();

    if notes.is_empty() {
        return format!("{}", "No notes found.".yellow());
    }

    let mut builder = Builder::default();
    builder.push_record(["Key", "Note (first 80 chars)"]);
    for note in notes {
        let key = note
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let raw = note
            .get("data")
            .and_then(|d| d.get("note"))
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        /* Strip basic HTML tags for display */
        let clean = strip_html(raw);
        builder.push_record([key, &truncate(&clean, NOTE_MAX_WIDTH)]);
    }
    builder.build()
        .with(Style::modern())
        .to_string()
}

pub fn collections_table(cols: &[ZoteroCollection]) -> String {
    if cols.is_empty() {
        return format!("{}", "No collections found.".yellow());
    }
    let mut builder = Builder::default();
    builder.push_record(["Key", "Name", "Parent"]);
    for col in cols {
        let parent = match &col.data.parent_collection {
            Some(p) if !p.is_null() && p != &serde_json::Value::Bool(false) => {
                p.as_str().unwrap_or("—").to_string()
            }
            _ => "—".to_string(),
        };
        builder.push_record([&col.data.key, &col.data.name, &parent]);
    }
    builder.build()
        .with(Style::modern())
        .to_string()
}

pub fn tags_table(tags: &[serde_json::Value]) -> String {
    if tags.is_empty() {
        return format!("{}", "No tags found.".yellow());
    }
    let mut builder = Builder::default();
    builder.push_record(["Tag", "Type"]);
    for tag in tags {
        let name = tag.get("tag").and_then(|v| v.as_str()).unwrap_or("—");
        let ttype = tag
            .get("type")
            .and_then(|v| v.as_u64())
            .map(|t| t.to_string())
            .unwrap_or_else(|| "0".to_string());
        builder.push_record([name, &ttype]);
    }
    builder.build()
        .with(Style::modern())
        .to_string()
}

/* ------------------------------------------------------------------ */
/*  Helpers                                                             */
/* ------------------------------------------------------------------ */

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", t)
    }
}

fn wrap(s: &str, width: usize) -> String {
    let mut out = String::new();
    let mut line_len = 0;
    for word in s.split_whitespace() {
        if line_len + word.len() + 1 > width && line_len > 0 {
            out.push('\n');
            line_len = 0;
        }
        if line_len > 0 {
            out.push(' ');
            line_len += 1;
        }
        out.push_str(word);
        line_len += word.len();
    }
    out
}

fn strip_html(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}
