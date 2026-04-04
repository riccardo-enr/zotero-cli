use serde::{Deserialize, Serialize};

/* Zotero API item data as returned by the local connector API. Only the
   fields we care about are declared; unknown fields are ignored. */

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ItemData {
    pub key: String,
    pub title: Option<String>,
    pub item_type: Option<String>,
    pub date: Option<String>,
    pub abstract_note: Option<String>,
    #[serde(default)]
    pub creators: Vec<Creator>,
    #[serde(default)]
    pub tags: Vec<Tag>,
    pub doi: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Creator {
    #[serde(rename = "creatorType")]
    pub creator_type: Option<String>,
    #[serde(rename = "firstName")]
    pub first_name: Option<String>,
    #[serde(rename = "lastName")]
    pub last_name: Option<String>,
    pub name: Option<String>,
}

impl Creator {
    pub fn display_name(&self) -> String {
        match (&self.last_name, &self.first_name, &self.name) {
            (Some(last), Some(first), _) => format!("{last}, {first}"),
            (Some(last), None, _) => last.clone(),
            (None, None, Some(name)) => name.clone(),
            _ => String::from("Unknown"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Tag {
    pub tag: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ZoteroItem {
    pub key: String,
    pub data: ItemData,
}

/* Compact representation for list commands — strips verbose fields (abstract,
   url, doi, tags) to reduce JSON payload when piping to an LLM. */
#[derive(Debug, Serialize)]
pub struct CompactItem {
    pub key: String,
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub item_type: Option<String>,
    pub date: Option<String>,
    pub authors: Vec<String>,
}

impl CompactItem {
    pub fn from_item(item: &ZoteroItem) -> Self {
        let authors = item
            .data
            .creators
            .iter()
            .filter(|c| c.creator_type.as_deref() == Some("author"))
            .map(|c| c.display_name())
            .collect();
        CompactItem {
            key: item.key.clone(),
            title: item.data.title.clone(),
            item_type: item.data.item_type.clone(),
            date: item.data.date.clone(),
            authors,
        }
    }
}

/* Collection */
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CollectionData {
    pub key: String,
    pub name: String,
    #[serde(rename = "parentCollection")]
    pub parent_collection: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ZoteroCollection {
    pub key: String,
    pub data: CollectionData,
}
