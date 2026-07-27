//! Yahoo! JAPAN keyword autocomplete.
//!
//! The suggest backend wants an `appid`. That value is *not* a user secret —
//! Yahoo embeds it verbatim in the HTML of its own public search page, which is
//! where the browser search box reads it from too. We do the same: scrape it on
//! first use, cache it for the process lifetime, and fall back to the last
//! known-good value if the page layout shifts. That keeps the server zero-config.

use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use crate::common::*;
use crate::YahooSearchClient;

const ENDPOINT: &str =
    "https://n-assist-search.yahooapis.jp/SuggestSearchService/V5/webassistSearch";
const REFERER: &str = "https://search.yahoo.co.jp/";
/// Page scraped for the public `appid`; any query works, so keep it trivial.
const APPID_SOURCE: &str = "https://search.yahoo.co.jp/search?p=a&ei=UTF-8";
/// Last known-good public appid, used only if scraping fails.
const FALLBACK_APPID: &str = "dj0zaiZpPVU5MGlSOUZ4cHVLbCZzPWNvbnN1bWVyc2VjcmV0Jng9ZGQ-";

static APPID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""appid"\s*:\s*"([A-Za-z0-9_\-]+)""#).unwrap());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub keyword: String,
    pub search_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestResult {
    pub provider: &'static str,
    pub vertical: &'static str,
    pub ok: bool,
    pub query: String,
    pub count: usize,
    pub suggestions: Vec<Suggestion>,
}

impl YahooSearchClient {
    pub async fn suggest_keywords(&self, query: &str, limit: i64) -> Result<SuggestResult> {
        let query = query.trim();
        if query.is_empty() {
            return Err(anyhow!("query must not be empty"));
        }
        let limit = clamp_limit(limit, 20);
        let appid = self.suggest_appid().await;

        let url = format!(
            "{ENDPOINT}?appid={}&query={}&output=json&device=pc",
            urlencode(&appid),
            urlencode(query)
        );
        let body = self.get_json(&url, REFERER).await?;

        let suggestions: Vec<Suggestion> = body
            .get("Result")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        let keyword = get_str(r, "Suggest")?;
                        let search_url = get_raw(r, "Url").unwrap_or_else(|| {
                            format!(
                                "https://search.yahoo.co.jp/search?p={}&ei=UTF-8",
                                urlencode(&keyword)
                            )
                        });
                        Some(Suggestion {
                            keyword,
                            search_url,
                        })
                    })
                    .take(limit)
                    .collect()
            })
            .unwrap_or_default();

        Ok(SuggestResult {
            provider: crate::PROVIDER,
            vertical: "suggest",
            ok: true,
            query: query.to_string(),
            count: suggestions.len(),
            suggestions,
        })
    }

    /// Resolve the public suggest appid, scraping once and caching thereafter.
    async fn suggest_appid(&self) -> String {
        let mut cached = self.suggest_appid.lock().await;
        if let Some(id) = cached.as_ref() {
            return id.clone();
        }
        let scraped = self
            .get_html(APPID_SOURCE, REFERER)
            .await
            .ok()
            .and_then(|html| extract_appid(&html))
            .unwrap_or_else(|| FALLBACK_APPID.to_string());
        *cached = Some(scraped.clone());
        scraped
    }
}

fn extract_appid(html: &str) -> Option<String> {
    APPID_RE
        .captures(html)
        .map(|c| c[1].to_string())
        .filter(|s| s.len() > 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_appid_reads_embedded_value() {
        let html = r#"...{"options":{"appid":"dj0zaiZpPVRFU1Q-","logid":"x"}}..."#;
        assert_eq!(extract_appid(html).as_deref(), Some("dj0zaiZpPVRFU1Q-"));
    }

    #[test]
    fn extract_appid_rejects_short_values() {
        assert!(extract_appid(r#"{"appid":"short"}"#).is_none());
        assert!(extract_appid("no appid here").is_none());
    }
}
