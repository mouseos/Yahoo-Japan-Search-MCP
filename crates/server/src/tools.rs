use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use yahoo_search::YahooSearchClient;

// ── Argument helpers ──

fn str_param(args: &Value, key: &str) -> String {
    args.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

/// The query, accepting the aliases agents most often reach for.
fn query_param(args: &Value) -> Result<String> {
    for key in ["query", "q", "p", "keyword"] {
        let v = str_param(args, key);
        if !v.is_empty() {
            return Ok(v);
        }
    }
    Err(anyhow!(
        "`query` is required and must be a non-empty string"
    ))
}

fn int_param(args: &Value, key: &str, default: i64) -> i64 {
    args.get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
        .unwrap_or(default)
}

fn tool_def(name: &str, description: &str, schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": schema,
    })
}

fn query_prop() -> Value {
    json!({"type": "string", "description": "Search query. Japanese works best; Yahoo! JAPAN indexes Japanese-language content far more deeply than other engines."})
}

// ── Tool definitions ──

pub fn tool_definitions() -> Vec<Value> {
    vec![
        tool_def(
            "yahoo_web_search",
            "Search the Yahoo! JAPAN web index. Best general-purpose tool for finding Japanese web pages, shops, official sites and documentation. Returns title, URL, snippet and the source site, plus related queries Yahoo suggests. 10 results per page.",
            json!({
                "type": "object",
                "properties": {
                    "query": query_prop(),
                    "page": {"type": "integer", "description": "1-based page number.", "default": 1},
                    "limit": {"type": "integer", "description": "Max results to return (1-10).", "default": 10},
                    "updated": {
                        "type": "string",
                        "description": "Restrict by last update. Yahoo offers only these windows — there is no month option.",
                        "enum": ["all", "day", "week", "year"],
                        "default": "all"
                    },
                    "site": {"type": "string", "description": "Restrict to one host, e.g. \"go.jp\" or \"example.co.jp\". Applied as a site: operator.", "default": ""}
                },
                "required": ["query"]
            }),
        ),
        tool_def(
            "yahoo_image_search",
            "Search Yahoo! JAPAN images. Returns the source page, the original image URL, a Yahoo-cached copy that stays reachable when the origin blocks hotlinking, a thumbnail and pixel dimensions. 20 results per page.",
            json!({
                "type": "object",
                "properties": {
                    "query": query_prop(),
                    "page": {"type": "integer", "description": "1-based page number.", "default": 1},
                    "limit": {"type": "integer", "description": "Max results to return (1-20).", "default": 20}
                },
                "required": ["query"]
            }),
        ),
        tool_def(
            "yahoo_video_search",
            "Search Yahoo! JAPAN videos across YouTube, Instagram, TikTok, Niconico and other hosts. Returns title, watch URL, uploader, duration, upload date and a summary. 20 results per page.",
            json!({
                "type": "object",
                "properties": {
                    "query": query_prop(),
                    "page": {"type": "integer", "description": "1-based page number.", "default": 1},
                    "limit": {"type": "integer", "description": "Max results to return (1-20).", "default": 20}
                },
                "required": ["query"]
            }),
        ),
        tool_def(
            "yahoo_news_search",
            "Search Yahoo!ニュース for Japanese news articles. Returns headline, article URL, publisher, publish time, category and a snippet. Use this over web search for current events in Japan. Yahoo returns one batch of up to 60 articles and does not paginate.",
            json!({
                "type": "object",
                "properties": {
                    "query": query_prop(),
                    "limit": {"type": "integer", "description": "Max articles to return (1-60).", "default": 20}
                },
                "required": ["query"]
            }),
        ),
        tool_def(
            "yahoo_realtime_search",
            "Search Yahoo!リアルタイム検索 for X (Twitter) posts. Best for what people in Japan are saying right now — breaking incidents, service outages, live reactions. Returns post text, author, timestamp and engagement counts. 40 results per page.",
            json!({
                "type": "object",
                "properties": {
                    "query": query_prop(),
                    "page": {"type": "integer", "description": "1-based page number.", "default": 1},
                    "limit": {"type": "integer", "description": "Max posts to return (1-40).", "default": 20},
                    "sort": {
                        "type": "string",
                        "description": "\"recent\" (newest first, default) or \"popular\" (most engagement).",
                        "enum": ["recent", "popular"],
                        "default": "recent"
                    }
                },
                "required": ["query"]
            }),
        ),
        tool_def(
            "yahoo_chiebukuro_search",
            "Search Yahoo!知恵袋, Japan's largest community Q&A site. Best for real-world Japanese consumer questions, troubleshooting and local know-how that rarely appears on formal sites. Returns question title, URL, status, answer/view counts and category path. 10 results per page.",
            json!({
                "type": "object",
                "properties": {
                    "query": query_prop(),
                    "page": {"type": "integer", "description": "1-based page number.", "default": 1},
                    "limit": {"type": "integer", "description": "Max questions to return (1-10).", "default": 10},
                    "status": {
                        "type": "string",
                        "description": "Filter by question state: all, open (回答受付中), vote (投票受付中), solved (解決済み).",
                        "enum": ["all", "open", "vote", "solved"],
                        "default": "all"
                    }
                },
                "required": ["query"]
            }),
        ),
        tool_def(
            "yahoo_suggest_keywords",
            "Get Yahoo! JAPAN autocomplete suggestions for a partial query. Useful for discovering how Japanese users actually phrase a topic before running a full search.",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Partial query to complete."},
                    "limit": {"type": "integer", "description": "Max suggestions to return (1-20).", "default": 10}
                },
                "required": ["query"]
            }),
        ),
        tool_def(
            "yahoo_list_verticals",
            "List the Yahoo! JAPAN search verticals this server exposes, with the tool name and paging behaviour of each. Call this to decide which search tool fits a question.",
            json!({"type": "object", "properties": {}}),
        ),
    ]
}

// ── Dispatch ──

pub async fn dispatch(name: &str, args: &Value, client: &YahooSearchClient) -> Result<Value> {
    match name {
        "yahoo_web_search" => {
            let result = client
                .web_search(
                    &query_param(args)?,
                    int_param(args, "page", 1),
                    int_param(args, "limit", 10),
                    &str_param(args, "updated"),
                    &str_param(args, "site"),
                )
                .await?;
            Ok(serde_json::to_value(result)?)
        }
        "yahoo_image_search" => {
            let result = client
                .image_search(
                    &query_param(args)?,
                    int_param(args, "page", 1),
                    int_param(args, "limit", 20),
                )
                .await?;
            Ok(serde_json::to_value(result)?)
        }
        "yahoo_video_search" => {
            let result = client
                .video_search(
                    &query_param(args)?,
                    int_param(args, "page", 1),
                    int_param(args, "limit", 20),
                )
                .await?;
            Ok(serde_json::to_value(result)?)
        }
        "yahoo_news_search" => {
            let result = client
                .news_search(&query_param(args)?, int_param(args, "limit", 20))
                .await?;
            Ok(serde_json::to_value(result)?)
        }
        "yahoo_realtime_search" => {
            let result = client
                .realtime_search(
                    &query_param(args)?,
                    int_param(args, "page", 1),
                    int_param(args, "limit", 20),
                    &str_param(args, "sort"),
                )
                .await?;
            Ok(serde_json::to_value(result)?)
        }
        "yahoo_chiebukuro_search" => {
            let result = client
                .chiebukuro_search(
                    &query_param(args)?,
                    int_param(args, "page", 1),
                    int_param(args, "limit", 10),
                    &str_param(args, "status"),
                )
                .await?;
            Ok(serde_json::to_value(result)?)
        }
        "yahoo_suggest_keywords" => {
            let result = client
                .suggest_keywords(&query_param(args)?, int_param(args, "limit", 10))
                .await?;
            Ok(serde_json::to_value(result)?)
        }
        "yahoo_list_verticals" => Ok(yahoo_search::verticals()),
        other => Err(anyhow!("Unknown tool: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_param_accepts_aliases() {
        assert_eq!(query_param(&json!({"q": "ラーメン"})).unwrap(), "ラーメン");
        assert_eq!(query_param(&json!({"keyword": " 東京 "})).unwrap(), "東京");
        assert!(query_param(&json!({"query": "  "})).is_err());
        assert!(query_param(&json!({})).is_err());
    }

    #[test]
    fn int_param_parses_numeric_strings() {
        assert_eq!(int_param(&json!({"limit": "5"}), "limit", 10), 5);
        assert_eq!(int_param(&json!({"limit": 3}), "limit", 10), 3);
        assert_eq!(int_param(&json!({}), "limit", 10), 10);
    }

    #[test]
    fn every_tool_has_a_schema_and_description() {
        for tool in tool_definitions() {
            assert!(!tool["name"].as_str().unwrap().is_empty());
            assert!(tool["description"].as_str().unwrap().len() > 40);
            assert_eq!(tool["inputSchema"]["type"], "object");
        }
    }
}
