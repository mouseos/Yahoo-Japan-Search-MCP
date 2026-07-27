//! Yahoo!知恵袋 Q&A search (`https://chiebukuro.yahoo.co.jp/search`).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::common::*;
use crate::{SearchResult, YahooSearchClient};

const BASE: &str = "https://chiebukuro.yahoo.co.jp/search";
const REFERER: &str = "https://chiebukuro.yahoo.co.jp/";
const PER_PAGE: i64 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChiebukuroItem {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// `open` (回答受付中), `vote` (投票受付中) or `solved` (解決済み).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posted_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub answer_count: i64,
    pub view_count: i64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
}

/// `status` argument → Yahoo's `type` parameter.
fn status_param(status: &str) -> Option<&'static str> {
    // Values come from the page's own status tabs (`questionStatusTab[].key`);
    // an unrecognised `flg` is silently ignored by Yahoo, so map strictly.
    match status.trim().to_lowercase().as_str() {
        "open" | "answering" | "active" => Some("0"),
        "solved" | "resolved" => Some("1"),
        "vote" | "voting" => Some("2"),
        _ => None,
    }
}

impl YahooSearchClient {
    pub async fn chiebukuro_search(
        &self,
        query: &str,
        page: i64,
        limit: i64,
        status: &str,
    ) -> Result<SearchResult<ChiebukuroItem>> {
        let page = page.max(1);
        let limit = clamp_limit(limit, PER_PAGE);
        let offset = (page - 1) * PER_PAGE + 1;

        let mut url = format!("{BASE}?p={}&b={offset}", urlencode(query.trim()));
        if let Some(flg) = status_param(status) {
            url.push_str(&format!("&flg={flg}"));
        }

        let html = self.get_html(&url, REFERER).await?;
        let props = extract_window_props(&html)?;

        let items: Vec<ChiebukuroItem> = get_path(&props, &["listSearchResults", "listContents"])
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(parse_question).take(limit).collect())
            .unwrap_or_default();

        let pagination = props.get("pagination").cloned().unwrap_or(Value::Null);
        let total_hits = get_i64(&pagination, "totalHits");
        let has_next = match (
            get_i64(&pagination, "currentPage"),
            get_i64(&pagination, "totalNumberOfPages"),
        ) {
            (Some(current), Some(total)) => current < total,
            // Fall back to the "next page" link the page renders for itself.
            _ => get_path(&props, &["template", "nextLink"])
                .map(|v| !v.is_null())
                .unwrap_or(false),
        };

        let related = get_path(&props, &["listKeywords", "items"])
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|k| {
                        k.as_str()
                            .map(clean_text)
                            .or_else(|| get_str(k, "keyword"))
                            .or_else(|| get_str(k, "text"))
                            .or_else(|| get_str(k, "name"))
                    })
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        Ok(SearchResult::new("chiebukuro", query, page, items)
            .with_total(total_hits)
            .with_next_page(has_next.then_some(page + 1))
            .with_related(related))
    }
}

fn parse_question(q: &Value) -> Option<ChiebukuroItem> {
    let url = get_raw(q, "url")?;
    Some(ChiebukuroItem {
        id: get_raw(q, "cid").unwrap_or_else(|| url.clone()),
        title: get_str(q, "heading").unwrap_or_default(),
        url,
        summary: get_str(q, "summary"),
        status: get_str(q, "status"),
        posted_at: get_str(q, "datePosted"),
        updated_at: get_str(q, "dateModified"),
        answer_count: get_i64(q, "numberAnswers").unwrap_or(0),
        view_count: get_i64(q, "numberViews").unwrap_or(0),
        categories: q
            .get("categories")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|c| get_str(c, "text")).collect())
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn status_param_maps_to_flg_values() {
        assert_eq!(status_param("open"), Some("0"));
        assert_eq!(status_param("solved"), Some("1"));
        assert_eq!(status_param("RESOLVED"), Some("1"));
        assert_eq!(status_param("vote"), Some("2"));
        assert_eq!(status_param("all"), None);
        assert_eq!(status_param(""), None);
    }

    #[test]
    fn parse_question_strips_em_tags() {
        let v = json!({
            "url": "https://detail.chiebukuro.yahoo.co.jp/qa/question_detail/q1",
            "cid": "q1",
            "heading": "<em>ラーメン</em>は好き？",
            "numberAnswers": 7
        });
        let item = parse_question(&v).unwrap();
        assert_eq!(item.title, "ラーメンは好き？");
        assert_eq!(item.answer_count, 7);
    }
}
