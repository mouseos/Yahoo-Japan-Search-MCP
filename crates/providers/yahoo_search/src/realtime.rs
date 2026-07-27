//! Yahoo! リアルタイム検索 — X (Twitter) post search
//! (`https://search.yahoo.co.jp/realtime/search`).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::common::*;
use crate::{SearchResult, YahooSearchClient};

const BASE: &str = "https://search.yahoo.co.jp/realtime/search";
const REFERER: &str = "https://search.yahoo.co.jp/realtime";
const PER_PAGE: i64 = 40;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeItem {
    pub id: String,
    pub url: String,
    pub text: String,
    pub author_name: String,
    pub author_handle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_url: Option<String>,
    /// Unix seconds, as Yahoo reports it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    pub reply_count: i64,
    pub repost_count: i64,
    pub like_count: i64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hashtags: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub media: Vec<String>,
}

/// `sort` argument → Yahoo's `md` parameter. Default is reverse-chronological.
fn sort_param(sort: &str) -> Option<&'static str> {
    match sort.trim().to_lowercase().as_str() {
        "popular" | "favorite" | "hot" => Some("h"),
        _ => None,
    }
}

impl YahooSearchClient {
    pub async fn realtime_search(
        &self,
        query: &str,
        page: i64,
        limit: i64,
        sort: &str,
    ) -> Result<SearchResult<RealtimeItem>> {
        let page = page.max(1);
        let limit = clamp_limit(limit, PER_PAGE);
        let offset = (page - 1) * PER_PAGE;

        let mut url = format!("{BASE}?p={}&ei=UTF-8&b={offset}", urlencode(query.trim()));
        if let Some(md) = sort_param(sort) {
            url.push_str(&format!("&md={md}"));
        }

        let html = self.get_html(&url, REFERER).await?;
        let data = extract_next_data(&html)?;

        let items: Vec<RealtimeItem> = get_path(
            &data,
            &["props", "pageProps", "pageData", "timeline", "entry"],
        )
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(parse_entry).take(limit).collect())
        .unwrap_or_default();

        let related = get_path(&data, &["props", "pageProps", "pageData", "relatedHashtag"])
            .map(parse_related_hashtags)
            .unwrap_or_default();

        let has_next = items.len() as i64 >= PER_PAGE;

        Ok(SearchResult::new("realtime", query, page, items)
            .with_next_page(has_next.then_some(page + 1))
            .with_related(related))
    }
}

fn parse_entry(entry: &Value) -> Option<RealtimeItem> {
    let id = get_raw(entry, "id")?;
    let url = get_raw(entry, "url")?;
    // `displayTextBody` excludes the trailing t.co link; fall back if absent.
    let text = entry
        .get("displayTextBody")
        .or_else(|| entry.get("displayText"))
        .and_then(|v| v.as_str())
        .map(clean_text)
        .unwrap_or_default();

    let hashtags = entry
        .get("hashtags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|h| h.as_str().map(clean_text))
                .collect()
        })
        .unwrap_or_default();

    let media = entry
        .get("media")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| get_path(m, &["item", "mediaUrl"]).and_then(|u| u.as_str()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    Some(RealtimeItem {
        id,
        url,
        text,
        author_name: get_str(entry, "name").unwrap_or_default(),
        author_handle: get_raw(entry, "screenName").unwrap_or_default(),
        author_url: get_raw(entry, "userUrl"),
        created_at: get_i64(entry, "createdAt"),
        reply_count: get_i64(entry, "replyCount").unwrap_or(0),
        repost_count: get_i64(entry, "rtCount").unwrap_or(0),
        like_count: get_i64(entry, "likesCount").unwrap_or(0),
        hashtags,
        media,
    })
}

fn parse_related_hashtags(node: &Value) -> Vec<String> {
    let list = node
        .get("hashtags")
        .or_else(|| node.get("items"))
        .or_else(|| node.get("list"))
        .and_then(|v| v.as_array());

    let Some(list) = list else { return Vec::new() };
    list.iter()
        .filter_map(|h| {
            h.as_str()
                .map(clean_text)
                .or_else(|| get_str(h, "hashtag"))
                .or_else(|| get_str(h, "text"))
                .or_else(|| get_str(h, "name"))
        })
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sort_param_only_maps_popular() {
        assert_eq!(sort_param("popular"), Some("h"));
        assert_eq!(sort_param("recent"), None);
        assert_eq!(sort_param(""), None);
    }

    #[test]
    fn parse_entry_strips_highlight_markers() {
        let v = json!({
            "id": "1",
            "url": "https://x.com/u/status/1",
            "displayTextBody": "美味しい\tSTART\tラーメン\tEND\tだった",
            "name": "テスト",
            "screenName": "test"
        });
        assert_eq!(parse_entry(&v).unwrap().text, "美味しいラーメンだった");
    }
}
