//! Yahoo! JAPAN search client.
//!
//! Every vertical is read from the server-rendered payload the public search
//! pages already ship to browsers (`__NEXT_DATA__`, `__PRELOADED_STATE__`,
//! `window.PROPS`). Nothing here needs an API key, a login or any user
//! configuration — the one credential involved, the keyword-suggest `appid`,
//! is the public value Yahoo embeds in its own search page and is scraped at
//! runtime (see [`suggest`]).

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub mod chiebukuro;
pub mod common;
pub mod image;
pub mod news;
pub mod realtime;
pub mod suggest;
pub mod video;
pub mod web;

use common::{ACCEPT_LANGUAGE, USER_AGENT};

pub const PROVIDER: &str = "yahoo_japan";

/// A vertical-agnostic search response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult<T> {
    pub provider: &'static str,
    pub vertical: &'static str,
    pub ok: bool,
    pub query: String,
    pub page: i64,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_hits: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page: Option<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub related_queries: Vec<String>,
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl<T> SearchResult<T> {
    pub fn new(vertical: &'static str, query: &str, page: i64, items: Vec<T>) -> Self {
        Self {
            provider: PROVIDER,
            vertical,
            ok: true,
            query: query.to_string(),
            page,
            count: items.len(),
            total_hits: None,
            next_page: None,
            related_queries: Vec::new(),
            items,
            note: None,
        }
    }

    pub fn with_total(mut self, total: Option<i64>) -> Self {
        self.total_hits = total;
        self
    }

    pub fn with_next_page(mut self, next: Option<i64>) -> Self {
        self.next_page = next;
        self
    }

    pub fn with_related(mut self, related: Vec<String>) -> Self {
        self.related_queries = related;
        self
    }

    pub fn with_note(mut self, note: &str) -> Self {
        self.note = Some(note.to_string());
        self
    }
}

/// Shared HTTP client. Cheap to clone; holds a connection pool and a cookie jar
/// (Yahoo hands out consent cookies that keep later requests from being
/// bounced to an interstitial).
#[derive(Clone)]
pub struct YahooSearchClient {
    http: Client,
    /// Cached keyword-suggest app id, scraped from the public search page.
    suggest_appid: std::sync::Arc<tokio::sync::Mutex<Option<String>>>,
}

impl Default for YahooSearchClient {
    fn default() -> Self {
        Self::new()
    }
}

impl YahooSearchClient {
    pub fn new() -> Self {
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .cookie_store(true)
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client");

        Self {
            http,
            suggest_appid: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// GET a Yahoo page as text, with the headers their edge expects.
    pub(crate) async fn get_html(&self, url: &str, referer: &str) -> Result<String> {
        let resp = self
            .http
            .get(url)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", ACCEPT_LANGUAGE)
            .header("Referer", referer)
            .header("Upgrade-Insecure-Requests", "1")
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.text().await?)
    }

    pub(crate) async fn get_json(&self, url: &str, referer: &str) -> Result<serde_json::Value> {
        let resp = self
            .http
            .get(url)
            .header("Accept", "application/json,text/javascript,*/*;q=0.01")
            .header("Accept-Language", ACCEPT_LANGUAGE)
            .header("Referer", referer)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
}

/// Static description of what this server can search — powers the
/// `yahoo_list_verticals` tool so an agent can pick the right one.
pub fn verticals() -> serde_json::Value {
    serde_json::json!({
        "provider": PROVIDER,
        "auth": "none",
        "config_required": false,
        "verticals": [
            {
                "id": "web",
                "name": "Yahoo!検索 (Web)",
                "tool": "yahoo_web_search",
                "description": "General web search. Supports paging and a last-updated filter.",
                "paginated": true
            },
            {
                "id": "image",
                "name": "Yahoo!画像検索",
                "tool": "yahoo_image_search",
                "description": "Image search with source page, dimensions and thumbnail URLs.",
                "paginated": true
            },
            {
                "id": "video",
                "name": "Yahoo!動画検索",
                "tool": "yahoo_video_search",
                "description": "Video search across YouTube, Instagram, TikTok, Niconico and others.",
                "paginated": true
            },
            {
                "id": "news",
                "name": "Yahoo!ニュース検索",
                "tool": "yahoo_news_search",
                "description": "Japanese news article search with publisher and publish time.",
                "paginated": false
            },
            {
                "id": "realtime",
                "name": "Yahoo!リアルタイム検索",
                "tool": "yahoo_realtime_search",
                "description": "Real-time X (Twitter) post search, sortable by recency or popularity.",
                "paginated": true
            },
            {
                "id": "chiebukuro",
                "name": "Yahoo!知恵袋検索",
                "tool": "yahoo_chiebukuro_search",
                "description": "Q&A search over Japan's largest community Q&A site.",
                "paginated": true
            },
            {
                "id": "suggest",
                "name": "Yahoo!検索サジェスト",
                "tool": "yahoo_suggest_keywords",
                "description": "Autocomplete keyword suggestions for a partial query.",
                "paginated": false
            }
        ]
    })
}
