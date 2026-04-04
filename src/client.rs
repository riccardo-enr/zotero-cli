use anyhow::{Context, Result};
use serde_json::Value;
use urlencoding::encode;

use crate::config::Config;
use crate::types::{ZoteroCollection, ZoteroItem};

/* ZoteroClient wraps the Zotero local connector API (localhost:23119/api).
Uses a synchronous HTTP client (ureq) — each CLI invocation makes exactly
one request to localhost so async provides no benefit and only adds runtime
cold-start overhead. */

pub struct ZoteroClient {
    http: ureq::Agent,
    base: String,
    api_key: Option<String>,
    user_id: Option<u64>,
    library_type: String,
}

impl ZoteroClient {
    pub fn new(cfg: &Config) -> Result<Self> {
        let http = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .build();
        Ok(ZoteroClient {
            http,
            base: cfg.api_base.clone(),
            api_key: cfg.api_key.clone(),
            user_id: cfg.user_id,
            library_type: cfg.library_type.clone(),
        })
    }

    fn auth(&self, req: ureq::Request) -> ureq::Request {
        if let Some(key) = &self.api_key {
            req.set("Zotero-API-Key", key)
        } else {
            req
        }
    }

    /* Build the library-scoped path prefix, e.g. /users/123 or /groups/456 */
    fn lib_path(&self) -> String {
        /* userID=0 is a special alias for the currently logged-in user's
        local library — always valid against the local connector API. */
        let id = self.user_id.unwrap_or(0);
        format!("/{}/{}", pluralise(&self.library_type), id)
    }

    fn get_body(&self, url: &str) -> Result<String> {
        let body = self
            .auth(self.http.get(url))
            .call()
            .map_err(|e| match e {
                ureq::Error::Status(code, resp) => {
                    let msg = resp.into_string().unwrap_or_default();
                    anyhow::anyhow!("Zotero API error {code}: {msg}")
                }
                e => anyhow::anyhow!("{e}"),
            })?
            .into_string()
            .context("reading response body")?;
        Ok(body)
    }

    /* ------------------------------------------------------------------ */
    /*  Core search / retrieval                                             */
    /* ------------------------------------------------------------------ */

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<ZoteroItem>> {
        let lib = self.lib_path();
        let url = format!(
            "{}{}/items?q={}&limit={}&v=3",
            self.base,
            lib,
            encode(query),
            limit
        );
        let body = self.get_body(&url)?;
        serde_json::from_str(&body).context("parsing search results")
    }

    pub fn get(&self, key: &str) -> Result<ZoteroItem> {
        let lib = self.lib_path();
        let url = format!("{}{}/items/{}?v=3", self.base, lib, key);
        let body = self.get_body(&url)?;
        serde_json::from_str(&body).context("parsing item")
    }

    /* ------------------------------------------------------------------ */
    /*  Children: annotations and notes                                     */
    /* ------------------------------------------------------------------ */

    pub fn children(&self, key: &str) -> Result<Vec<Value>> {
        let lib = self.lib_path();
        let url = format!("{}{}/items/{}/children?v=3", self.base, lib, key);
        let body = self.get_body(&url)?;
        serde_json::from_str(&body).context("parsing children")
    }

    /* ------------------------------------------------------------------ */
    /*  Collections                                                         */
    /* ------------------------------------------------------------------ */

    pub fn collections(&self) -> Result<Vec<ZoteroCollection>> {
        let lib = self.lib_path();
        let url = format!("{}{}/collections?v=3", self.base, lib);
        let body = self.get_body(&url)?;
        serde_json::from_str(&body).context("parsing collections")
    }

    pub fn collection_items(&self, id: &str) -> Result<Vec<ZoteroItem>> {
        let lib = self.lib_path();
        let url = format!("{}{}/collections/{}/items?v=3", self.base, lib, id);
        let body = self.get_body(&url)?;
        serde_json::from_str(&body).context("parsing collection items")
    }

    /* ------------------------------------------------------------------ */
    /*  Tags                                                                */
    /* ------------------------------------------------------------------ */

    pub fn tags(&self) -> Result<Vec<Value>> {
        let lib = self.lib_path();
        let url = format!("{}{}/tags?v=3", self.base, lib);
        let body = self.get_body(&url)?;
        serde_json::from_str(&body).context("parsing tags")
    }

    /* ------------------------------------------------------------------ */
    /*  Recent items                                                        */
    /* ------------------------------------------------------------------ */

    pub fn recent(&self, n: usize) -> Result<Vec<ZoteroItem>> {
        let lib = self.lib_path();
        let url = format!(
            "{}{}/items?sort=dateAdded&direction=desc&limit={}&v=3",
            self.base, lib, n
        );
        let body = self.get_body(&url)?;
        serde_json::from_str(&body).context("parsing recent")
    }

    /* ------------------------------------------------------------------ */
    /*  Add items                                                           */
    /* ------------------------------------------------------------------ */

    pub fn add_doi(&self, doi: &str) -> Result<Value> {
        let url = format!("{}/items?v=3", self.base);
        let payload = serde_json::json!([{
            "itemType": "journalArticle",
            "DOI": doi
        }]);
        let body = self
            .auth(self.http.post(&url))
            .send_json(payload)
            .map_err(|e| match e {
                ureq::Error::Status(code, resp) => {
                    let msg = resp.into_string().unwrap_or_default();
                    anyhow::anyhow!("Zotero API error {code}: {msg}")
                }
                e => anyhow::anyhow!("{e}"),
            })?
            .into_string()
            .context("reading response body")?;
        serde_json::from_str(&body).context("parsing add doi response")
    }

    pub fn add_url(&self, add_url: &str) -> Result<Value> {
        let translate_url = "http://localhost:1969/web";
        let payload = serde_json::json!({ "url": add_url, "sessionID": "zotero-cli" });
        let body = self
            .http
            .post(translate_url)
            .send_json(payload)
            .map_err(|e| match e {
                ureq::Error::Status(code, resp) => {
                    let msg = resp.into_string().unwrap_or_default();
                    anyhow::anyhow!("Zotero translator error {code}: {msg}")
                }
                e => anyhow::anyhow!("{e}"),
            })?
            .into_string()
            .context("reading response body")?;
        serde_json::from_str(&body).context("parsing add url response")
    }
}

fn pluralise(s: &str) -> &str {
    match s {
        "user" => "users",
        "group" => "groups",
        _ => s,
    }
}
