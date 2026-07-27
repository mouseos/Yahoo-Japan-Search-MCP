//! Yahoo! News Japan article search (`https://news.yahoo.co.jp/search`).
//!
//! Unlike the other verticals this page ships every result it is going to give
//! in one server-rendered batch (~60 articles) and ignores offset parameters —
//! the site itself paginates client-side. There is therefore no `page`
//! argument; callers get the full batch and slice it with `limit`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::common::*;
use crate::{SearchResult, YahooSearchClient};

const BASE: &str = "https://news.yahoo.co.jp/search";
const REFERER: &str = "https://news.yahoo.co.jp/";
const MAX_ITEMS: i64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsItem {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub article_type: Option<String>,
}

impl YahooSearchClient {
    pub async fn news_search(&self, query: &str, limit: i64) -> Result<SearchResult<NewsItem>> {
        let limit = clamp_limit(limit, MAX_ITEMS);

        let url = format!("{BASE}?p={}&ei=UTF-8", urlencode(query.trim()));
        let html = self.get_html(&url, REFERER).await?;
        let state = extract_preloaded_state(&html)?;

        let search = state.get("search").cloned().unwrap_or(Value::Null);
        let items: Vec<NewsItem> = search
            .get("contents")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(parse_news).take(limit).collect())
            .unwrap_or_default();

        Ok(
            SearchResult::new("news", query, 1, items)
                .with_total(get_i64(&search, "totalResults"))
                .with_note(
                    "Yahoo!ニュース検索 returns a single server-rendered batch (up to 60 articles) and does not support offset paging.",
                ),
        )
    }
}

fn parse_news(content: &Value) -> Option<NewsItem> {
    let url = get_raw(content, "permalink")?;
    let title = get_path(content, &["highlightSearchText", "headline"])
        .and_then(|v| v.as_str())
        .map(clean_text)
        .filter(|s| !s.is_empty())?;

    let published_at = get_path(content, &["publishTime"]).map(|t| {
        let date = t.get("date").and_then(|v| v.as_str()).unwrap_or("");
        let time = t.get("time").and_then(|v| v.as_str()).unwrap_or("");
        format!("{date} {time}").trim().to_string()
    });

    let categories = content
        .get("categories")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|c| get_str(c, "name")).collect())
        .unwrap_or_default();

    Some(NewsItem {
        id: get_raw(content, "contentId").unwrap_or_else(|| url.clone()),
        title,
        url,
        publisher: get_path(content, &["media", "name"])
            .and_then(|v| v.as_str())
            .map(clean_text),
        published_at: published_at.filter(|s| !s.is_empty()),
        summary: get_path(content, &["highlightSearchText", "body"])
            .and_then(|v| v.as_str())
            .map(clean_text)
            .filter(|s| !s.is_empty()),
        categories,
        thumbnail: get_path(content, &["thumbnail", "url"])
            .and_then(|v| v.as_str())
            .map(str::to_string),
        article_type: get_str(content, "newsArticleType"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_news_joins_date_and_time() {
        let v = json!({
            "permalink": "https://news.yahoo.co.jp/articles/abc",
            "contentId": "abc",
            "highlightSearchText": {"headline": "見出し\u{2}テスト\u{3}"},
            "publishTime": {"date": "7/27(月)", "time": "11:50"}
        });
        let item = parse_news(&v).unwrap();
        assert_eq!(item.title, "見出しテスト");
        assert_eq!(item.published_at.as_deref(), Some("7/27(月) 11:50"));
    }

    #[test]
    fn parse_news_requires_headline() {
        let v = json!({"permalink": "https://news.yahoo.co.jp/articles/abc"});
        assert!(parse_news(&v).is_none());
    }
}
