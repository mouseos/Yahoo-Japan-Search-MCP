//! Yahoo! JAPAN video search (`https://search.yahoo.co.jp/video/search`).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::common::*;
use crate::{SearchResult, YahooSearchClient};

const BASE: &str = "https://search.yahoo.co.jp/video/search";
const REFERER: &str = "https://search.yahoo.co.jp/";
const PER_PAGE: i64 = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoItem {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uploader: Option<String>,
    /// Human-readable running time, e.g. `"12:34"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
}

impl YahooSearchClient {
    pub async fn video_search(
        &self,
        query: &str,
        page: i64,
        limit: i64,
    ) -> Result<SearchResult<VideoItem>> {
        let page = page.max(1);
        let limit = clamp_limit(limit, PER_PAGE);
        let offset = (page - 1) * PER_PAGE + 1;

        let url = format!("{BASE}?p={}&ei=UTF-8&b={offset}", urlencode(query.trim()));
        let html = self.get_html(&url, REFERER).await?;
        let data = extract_next_data(&html)?;

        let items: Vec<VideoItem> =
            get_path(&data, &["props", "initialProps", "pageProps", "algos"])
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(parse_video).take(limit).collect())
                .unwrap_or_default();

        let has_next = items.len() as i64 >= PER_PAGE;

        Ok(SearchResult::new("video", query, page, items)
            .with_next_page(has_next.then_some(page + 1)))
    }
}

fn parse_video(algo: &Value) -> Option<VideoItem> {
    let url = get_raw(algo, "refererUrl")?;
    Some(VideoItem {
        id: get_raw(algo, "id").unwrap_or_else(|| url.clone()),
        title: get_str(algo, "title").unwrap_or_else(|| url.clone()),
        url,
        source: get_str(algo, "source").or_else(|| get_str(algo, "siteName")),
        uploader: get_str(algo, "uploader"),
        duration: get_str(algo, "duration"),
        upload_date: get_str(algo, "uploadDate"),
        summary: get_str(algo, "summary"),
        thumbnail: get_path(algo, &["thumbnail", "url"])
            .and_then(|v| v.as_str())
            .map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_video_requires_referer_url() {
        assert!(parse_video(&json!({"title": "x"})).is_none());
    }

    #[test]
    fn parse_video_falls_back_to_site_name() {
        let v = json!({"refererUrl": "https://x/v", "siteName": "youtube.com"});
        assert_eq!(
            parse_video(&v).unwrap().source.as_deref(),
            Some("youtube.com")
        );
    }
}
