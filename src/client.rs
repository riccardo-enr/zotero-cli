use anyhow::{bail, Context, Result};
use reqwest::{Client, RequestBuilder};
use serde_json::Value;

use crate::config::Config;
use crate::types::{ZoteroCollection, ZoteroItem};

/* ZoteroClient wraps the Zotero local connector API (localhost:23119/api).
   Every method returns raw JSON values so callers can choose between
   human-readable table output and --json passthrough without re-serialising. */

pub struct ZoteroClient {
    http: Client,
    base: String,
    api_key: Option<String>,
    user_id: Option<u64>,
    library_type: String,
}

impl ZoteroClient {
    pub fn new(cfg: &Config) -> Result<Self> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("building HTTP client")?;
        Ok(ZoteroClient {
            http,
            base: cfg.api_base.clone(),
            api_key: cfg.api_key.clone(),
            user_id: cfg.user_id,
            library_type: cfg.library_type.clone(),
        })
    }

    fn auth(&self, req: RequestBuilder) -> RequestBuilder {
        if let Some(key) = &self.api_key {
            req.header("Zotero-API-Key", key)
        } else {
            req
        }
    }

    /* Build the library-scoped path prefix, e.g. /users/123 or /groups/456 */
    fn lib_path(&self) -> Result<String> {
        match self.user_id {
            Some(id) => Ok(format!("/{}/{}", pluralise(&self.library_type), id)),
            None => {
                /* Fall back to the local connector's /api path which does not
                   require a user id — works for the default local library. */
                Ok(String::new())
            }
        }
    }

    /* ------------------------------------------------------------------ */
    /*  Core search / retrieval                                             */
    /* ------------------------------------------------------------------ */

    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<ZoteroItem>> {
        let lib = self.lib_path()?;
        let url = format!("{}{}/items?q={}&limit={}&v=3", self.base, lib, query, limit);
        let resp = self
            .auth(self.http.get(&url))
            .send()
            .await
            .context("GET items")?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("Zotero API error {status}: {body}");
        }
        let items: Vec<ZoteroItem> =
            serde_json::from_str(&body).context("parsing search results")?;
        Ok(items)
    }

    pub async fn get(&self, key: &str) -> Result<ZoteroItem> {
        let lib = self.lib_path()?;
        let url = format!("{}{}/items/{}?v=3", self.base, lib, key);
        let resp = self
            .auth(self.http.get(&url))
            .send()
            .await
            .context("GET item")?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("Zotero API error {status}: {body}");
        }
        let item: ZoteroItem = serde_json::from_str(&body).context("parsing item")?;
        Ok(item)
    }

    /* ------------------------------------------------------------------ */
    /*  Children: annotations and notes                                     */
    /* ------------------------------------------------------------------ */

    pub async fn children(&self, key: &str) -> Result<Vec<Value>> {
        let lib = self.lib_path()?;
        let url = format!("{}{}/items/{}/children?v=3", self.base, lib, key);
        let resp = self
            .auth(self.http.get(&url))
            .send()
            .await
            .context("GET children")?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("Zotero API error {status}: {body}");
        }
        let children: Vec<Value> = serde_json::from_str(&body).context("parsing children")?;
        Ok(children)
    }

    /* ------------------------------------------------------------------ */
    /*  Collections                                                         */
    /* ------------------------------------------------------------------ */

    pub async fn collections(&self) -> Result<Vec<ZoteroCollection>> {
        let lib = self.lib_path()?;
        let url = format!("{}{}/collections?v=3", self.base, lib);
        let resp = self
            .auth(self.http.get(&url))
            .send()
            .await
            .context("GET collections")?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("Zotero API error {status}: {body}");
        }
        let cols: Vec<ZoteroCollection> =
            serde_json::from_str(&body).context("parsing collections")?;
        Ok(cols)
    }

    pub async fn collection_items(&self, id: &str) -> Result<Vec<ZoteroItem>> {
        let lib = self.lib_path()?;
        let url = format!("{}{}/collections/{}/items?v=3", self.base, lib, id);
        let resp = self
            .auth(self.http.get(&url))
            .send()
            .await
            .context("GET collection items")?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("Zotero API error {status}: {body}");
        }
        let items: Vec<ZoteroItem> =
            serde_json::from_str(&body).context("parsing collection items")?;
        Ok(items)
    }

    /* ------------------------------------------------------------------ */
    /*  Tags                                                                */
    /* ------------------------------------------------------------------ */

    pub async fn tags(&self) -> Result<Vec<Value>> {
        let lib = self.lib_path()?;
        let url = format!("{}{}/tags?v=3", self.base, lib);
        let resp = self
            .auth(self.http.get(&url))
            .send()
            .await
            .context("GET tags")?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("Zotero API error {status}: {body}");
        }
        let tags: Vec<Value> = serde_json::from_str(&body).context("parsing tags")?;
        Ok(tags)
    }

    /* ------------------------------------------------------------------ */
    /*  Recent items                                                        */
    /* ------------------------------------------------------------------ */

    pub async fn recent(&self, n: usize) -> Result<Vec<ZoteroItem>> {
        let lib = self.lib_path()?;
        let url = format!(
            "{}{}/items?sort=dateAdded&direction=desc&limit={}&v=3",
            self.base, lib, n
        );
        let resp = self
            .auth(self.http.get(&url))
            .send()
            .await
            .context("GET recent")?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("Zotero API error {status}: {body}");
        }
        let items: Vec<ZoteroItem> = serde_json::from_str(&body).context("parsing recent")?;
        Ok(items)
    }

    /* ------------------------------------------------------------------ */
    /*  Add items                                                           */
    /* ------------------------------------------------------------------ */

    pub async fn add_doi(&self, doi: &str) -> Result<Value> {
        /* The Zotero local connector exposes /api/items/newItem-style endpoints.
           We use the web-based translate service bundled in the connector. */
        let url = format!("{}/items?v=3", self.base);
        let payload = serde_json::json!([{
            "itemType": "journalArticle",
            "DOI": doi
        }]);
        let resp = self
            .auth(self.http.post(&url).json(&payload))
            .send()
            .await
            .context("POST add doi")?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("Zotero API error {status}: {body}");
        }
        let v: Value = serde_json::from_str(&body)?;
        Ok(v)
    }

    pub async fn add_url(&self, add_url: &str) -> Result<Value> {
        let translate_url = format!("http://localhost:1969/web");
        let payload = serde_json::json!({ "url": add_url, "sessionID": "zotero-cli" });
        let resp = self
            .http
            .post(&translate_url)
            .json(&payload)
            .send()
            .await
            .context("POST translate url")?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            bail!("Zotero translator error {status}: {body}");
        }
        let v: Value = serde_json::from_str(&body)?;
        Ok(v)
    }
}

fn pluralise(s: &str) -> &str {
    match s {
        "user" => "users",
        "group" => "groups",
        _ => s,
    }
}
