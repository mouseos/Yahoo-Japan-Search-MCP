use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use yahoo_search::{YahooSearchClient, PROVIDER};

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

// ── Error form (unified with Baidu_MCP) ──
//
// A search that *ran* but failed on Yahoo's side (HTTP error, timeout, a
// page layout change, …) is reported as a structured tool result —
// `{provider, ok:false, vertical, error:{kind, message}}` — the same form
// Baidu_MCP uses, so agents can branch on `ok` uniformly across search
// MCPs. Argument errors (missing query) and unknown tools are not search
// failures: they still surface as MCP `isError` results via `Err` here.
// Successful results keep their existing shape untouched.

/// Classify a provider error into a stable `kind` string.
fn error_kind(e: &anyhow::Error) -> &'static str {
    for cause in e.chain() {
        if let Some(re) = cause.downcast_ref::<reqwest::Error>() {
            if re.is_timeout() {
                return "timeout";
            }
            if re.is_connect() {
                return "connection_error";
            }
            if re.is_status() {
                return "http_error";
            }
            if re.is_decode() || re.is_body() {
                return "parse_error";
            }
            return "http_error";
        }
    }
    if e.to_string()
        .contains("not found (Yahoo page layout may have changed)")
    {
        "parse_error"
    } else {
        "search_error"
    }
}

/// The unified structured error result for a failed search.
fn error_result(vertical: &str, e: anyhow::Error) -> Value {
    json!({
        "provider": PROVIDER,
        "ok": false,
        "vertical": vertical,
        "error": {
            "kind": error_kind(&e),
            "message": e.to_string(),
        }
    })
}

pub async fn dispatch(name: &str, args: &Value, client: &YahooSearchClient) -> Result<Value> {
    match name {
        "yahoo_web_search" => match client
            .web_search(
                &query_param(args)?,
                int_param(args, "page", 1),
                int_param(args, "limit", 10),
                &str_param(args, "updated"),
                &str_param(args, "site"),
            )
            .await
        {
            Ok(result) => Ok(serde_json::to_value(result)?),
            Err(e) => Ok(error_result("web", e)),
        },
        "yahoo_image_search" => match client
            .image_search(
                &query_param(args)?,
                int_param(args, "page", 1),
                int_param(args, "limit", 20),
            )
            .await
        {
            Ok(result) => Ok(serde_json::to_value(result)?),
            Err(e) => Ok(error_result("image", e)),
        },
        "yahoo_video_search" => match client
            .video_search(
                &query_param(args)?,
                int_param(args, "page", 1),
                int_param(args, "limit", 20),
            )
            .await
        {
            Ok(result) => Ok(serde_json::to_value(result)?),
            Err(e) => Ok(error_result("video", e)),
        },
        "yahoo_news_search" => match client
            .news_search(&query_param(args)?, int_param(args, "limit", 20))
            .await
        {
            Ok(result) => Ok(serde_json::to_value(result)?),
            Err(e) => Ok(error_result("news", e)),
        },
        "yahoo_realtime_search" => match client
            .realtime_search(
                &query_param(args)?,
                int_param(args, "page", 1),
                int_param(args, "limit", 20),
                &str_param(args, "sort"),
            )
            .await
        {
            Ok(result) => Ok(serde_json::to_value(result)?),
            Err(e) => Ok(error_result("realtime", e)),
        },
        "yahoo_chiebukuro_search" => match client
            .chiebukuro_search(
                &query_param(args)?,
                int_param(args, "page", 1),
                int_param(args, "limit", 10),
                &str_param(args, "status"),
            )
            .await
        {
            Ok(result) => Ok(serde_json::to_value(result)?),
            Err(e) => Ok(error_result("chiebukuro", e)),
        },
        "yahoo_suggest_keywords" => match client
            .suggest_keywords(&query_param(args)?, int_param(args, "limit", 10))
            .await
        {
            Ok(result) => Ok(serde_json::to_value(result)?),
            Err(e) => Ok(error_result("suggest", e)),
        },
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
    fn error_result_uses_the_unified_form() {
        let v = error_result("web", anyhow!("boom"));
        assert_eq!(v["provider"], "yahoo_japan");
        assert_eq!(v["ok"], false);
        assert_eq!(v["vertical"], "web");
        assert_eq!(v["error"]["kind"], "search_error");
        assert_eq!(v["error"]["message"], "boom");
    }

    #[test]
    fn error_kind_classifies_layout_changes_as_parse_error() {
        let e = anyhow!("__NEXT_DATA__ block not found (Yahoo page layout may have changed)");
        assert_eq!(error_kind(&e), "parse_error");
        let v = error_result("news", e);
        assert_eq!(v["error"]["kind"], "parse_error");
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
