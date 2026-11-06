//! Yahoo! JAPAN search client — usable both as the search engine of the
//! Yahoo-Japan-Search-MCP server and as a standalone Rust library.
//!
//! Every vertical is read from the server-rendered payload the public search
//! pages already ship to browsers (`__NEXT_DATA__`, `__PRELOADED_STATE__`,
//! `window.PROPS`). Nothing here needs an API key, a login or any user
//! configuration — the one credential involved, the keyword-suggest `appid`,
//! is the public value Yahoo embeds in its own search page and is scraped at
//! runtime (see [`suggest`]).
//!
//! # Library usage
//!
//! Add the provider crate to your `Cargo.toml` (the library is imported as
//! `yahoo_search`, after the `[lib]` target name):
//!
//! ```toml
//! [dependencies]
//! yahoo_search = { git = "https://github.com/mouseos/Yahoo-Japan-Search-MCP", package = "yahoo-search-mcp-provider" }
//! tokio = { version = "1", features = ["full"] }
//! anyhow = "1"
//! ```
//!
//! All searches go through [`YahooSearchClient`]; every method returns
//! `anyhow::Result`, wrapping either a [`SearchResult`] of the vertical's
//! item type or, for autocomplete, a [`SuggestResult`]:
//!
//! ```no_run
//! # fn main() {
//! # let rt = tokio::runtime::Builder::new_current_thread()
//! #     .enable_all().build().unwrap();
//! # rt.block_on(async {
//! use yahoo_search::YahooSearchClient;
//!
//! let client = YahooSearchClient::new();
//! // Web search: query, page (1-based), limit, updated filter, site filter.
//! let result = client.web_search("味噌ラーメン 作り方", 1, 5, "", "").await.unwrap();
//! assert!(result.ok);
//! for item in &result.items {
//!     println!("{} — {}", item.title, item.url);
//! }
//! // Keyword autocomplete.
//! let suggest = client.suggest_keywords("ラーメン", 10).await.unwrap();
//! for s in &suggest.suggestions {
//!     println!("{}", s.keyword);
//! }
//! # });
//! # }
//! ```
//!
//! The result types and the offline helpers are also available without any
//! network access — handy for building values, serialising results and
//! extracting the payload from a saved search page:
//!
//! ```
//! use yahoo_search::{SearchResult, WebItem, PROVIDER};
//!
//! let item = WebItem {
//!     rank: 1,
//!     title: "Example".to_string(),
//!     url: "https://example.jp/".to_string(),
//!     description: None,
//!     site: None,
//! };
//! let result = SearchResult::new("web", "クエリ", 1, vec![item]).with_total(Some(42));
//! assert_eq!(result.provider, PROVIDER);
//! assert_eq!(result.count, 1);
//! assert_eq!(yahoo_search::urlencode("a b"), "a%20b");
//! ```

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

// ── Root re-exports: the stable library surface ──
//
// Library users should only need `use yahoo_search::{...}`. The per-vertical
// modules stay public for compatibility, but every item type, the suggest
// types and the offline payload/text helpers are re-exported here so callers
// never have to reach into a module path.

/// Q&A item returned by [`YahooSearchClient::chiebukuro_search`].
pub use chiebukuro::ChiebukuroItem;
/// Offline helpers: payload extraction from saved pages and text cleanup.
pub use common::{
    clean_text, decode_entities, extract_next_data, extract_preloaded_state, extract_window_props,
    urlencode,
};
/// Image item (and its size variants) returned by
/// [`YahooSearchClient::image_search`].
pub use image::{ImageItem, ImageSize};
/// News article item returned by [`YahooSearchClient::news_search`].
pub use news::NewsItem;
/// Real-time (X/Twitter) post item returned by
/// [`YahooSearchClient::realtime_search`].
pub use realtime::RealtimeItem;
/// Autocomplete types returned by [`YahooSearchClient::suggest_keywords`].
pub use suggest::{SuggestResult, Suggestion};
/// Video item returned by [`YahooSearchClient::video_search`].
pub use video::VideoItem;
/// Web item returned by [`YahooSearchClient::web_search`].
pub use web::WebItem;

/// Provider identifier used in every result envelope (`"yahoo_japan"`).
pub const PROVIDER: &str = "yahoo_japan";

/// A vertical-agnostic search response envelope.
///
/// `provider` / `vertical` are owned `String`s (not `&'static str`) so that
/// downstream users can deserialise results back — e.g. from
/// `serde_json::Value` or a cache file — which a borrowed field makes
/// impossible outside `'static` lifetimes. The JSON form is unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult<T> {
    pub provider: String,
    pub vertical: String,
    pub ok: bool,
    pub query: String,
    pub page: i64,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_hits: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_queries: Vec<String>,
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl<T> SearchResult<T> {
    /// Build a successful envelope for `vertical` from already-fetched items.
    /// `count` is derived from `items.len()`; chain the `with_*` builders to
    /// attach totals, paging and related queries.
    pub fn new(vertical: &str, query: &str, page: i64, items: Vec<T>) -> Self {
        Self {
            provider: PROVIDER.to_string(),
            vertical: vertical.to_string(),
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

    /// Attach the total hit count reported by Yahoo, if known.
    pub fn with_total(mut self, total: Option<i64>) -> Self {
        self.total_hits = total;
        self
    }

    /// Attach the next 1-based page number, if a further page exists.
    pub fn with_next_page(mut self, next: Option<i64>) -> Self {
        self.next_page = next;
        self
    }

    /// Attach related queries suggested alongside the results.
    pub fn with_related(mut self, related: Vec<String>) -> Self {
        self.related_queries = related;
        self
    }

    /// Attach a human-readable note about the result (e.g. a paging caveat).
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
    /// Create a client with the shared connection pool and cookie jar.
    /// No configuration, API key or login is required. Cheap to clone and
    /// safe to share across tasks; one client per process is the norm.
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

/// Static description of the searchable verticals (web, image, video, news,
/// realtime, chiebukuro, suggest) as a JSON value — powers the
/// `yahoo_list_verticals` MCP tool so an agent can pick the right one, and
/// doubles as machine-readable documentation for library users.
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
