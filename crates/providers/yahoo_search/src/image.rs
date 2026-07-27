//! Yahoo! JAPAN image search (`https://search.yahoo.co.jp/image/search`).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::common::*;
use crate::{SearchResult, YahooSearchClient};

const BASE: &str = "https://search.yahoo.co.jp/image/search";
const REFERER: &str = "https://search.yahoo.co.jp/";
const PER_PAGE: i64 = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSize {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageItem {
    pub id: String,
    pub title: String,
    /// Page the image was found on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_site: Option<String>,
    /// Image at its original host, when Yahoo exposes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original: Option<ImageSize>,
    /// Yahoo-cached copy — always reachable, even when the origin blocks hotlinks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached: Option<ImageSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<ImageSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_format: Option<String>,
}

impl YahooSearchClient {
    pub async fn image_search(
        &self,
        query: &str,
        page: i64,
        limit: i64,
    ) -> Result<SearchResult<ImageItem>> {
        let page = page.max(1);
        let limit = clamp_limit(limit, PER_PAGE);
        let offset = (page - 1) * PER_PAGE + 1;

        let url = format!("{BASE}?p={}&ei=UTF-8&b={offset}", urlencode(query.trim()));
        let html = self.get_html(&url, REFERER).await?;
        let data = extract_next_data(&html)?;

        let props = get_path(&data, &["props", "initialProps", "pageProps"])
            .cloned()
            .unwrap_or(Value::Null);

        let items: Vec<ImageItem> = props
            .get("algos")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(parse_image).take(limit).collect())
            .unwrap_or_default();

        let has_next = items.len() as i64 >= PER_PAGE
            || props.get("endIndex").and_then(|v| v.as_i64()).unwrap_or(0) >= offset + PER_PAGE - 1;

        Ok(SearchResult::new("image", query, page, items)
            .with_next_page(has_next.then_some(page + 1))
            .with_related(parse_related(&props)))
    }
}

fn parse_size(v: &Value, key: &str) -> Option<ImageSize> {
    let node = v.get(key)?;
    // Yahoo keeps dead entries in the payload behind an `isNotFound` flag.
    if node.get("isNotFound").and_then(|f| f.as_bool()) == Some(true) {
        return None;
    }
    Some(ImageSize {
        url: get_raw(node, "url")?,
        width: get_i64(node, "width"),
        height: get_i64(node, "height"),
    })
}

fn parse_image(algo: &Value) -> Option<ImageItem> {
    let id = get_raw(algo, "id")?;
    Some(ImageItem {
        title: get_str(algo, "title").unwrap_or_default(),
        id,
        source_url: get_raw(algo, "refererUrl"),
        source_site: get_str(algo, "refererName").or_else(|| get_str(algo, "refererDomain")),
        original: parse_size(algo, "original"),
        cached: parse_size(algo, "main"),
        thumbnail: parse_size(algo, "thumbnail"),
        file_format: get_str(algo, "fileFormat"),
    })
}

fn parse_related(props: &Value) -> Vec<String> {
    let Some(groups) = get_path(props, &["relatedSearch", "query"]).and_then(|v| v.as_array())
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for group in groups {
        let Some(words) = group.get("words").and_then(|v| v.as_array()) else {
            continue;
        };
        for w in words {
            if let Some(text) = get_str(w, "word") {
                if !out.contains(&text) {
                    out.push(text);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_size_drops_not_found_entries() {
        let v = json!({"main": {"url": "https://x/y.jpg", "isNotFound": true}});
        assert!(parse_size(&v, "main").is_none());
    }

    #[test]
    fn parse_size_reads_dimensions() {
        let v = json!({"main": {"url": "https://x/y.jpg", "width": 10, "height": 20}});
        let s = parse_size(&v, "main").unwrap();
        assert_eq!((s.width, s.height), (Some(10), Some(20)));
    }
}
