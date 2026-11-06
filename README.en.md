# Yahoo! JAPAN Search MCP

An MCP server that gives AI agents access to Yahoo! JAPAN search. **No API key, no login, no configuration.**

[日本語](README.md)

[![CI](https://github.com/mouseos/Yahoo-Japan-Search-MCP/actions/workflows/ci.yml/badge.svg)](https://github.com/mouseos/Yahoo-Japan-Search-MCP/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## Verticals

| Tool | Vertical | Per page |
|---|---|---|
| `yahoo_web_search` | Web search | 10 |
| `yahoo_image_search` | Image search | 20 |
| `yahoo_video_search` | Video search | 20 |
| `yahoo_news_search` | Yahoo! News Japan | up to 60 (no paging) |
| `yahoo_realtime_search` | Real-time X (Twitter) posts | 40 |
| `yahoo_chiebukuro_search` | Yahoo! Chiebukuro community Q&A | 10 |
| `yahoo_suggest_keywords` | Keyword autocomplete | 20 |
| `yahoo_list_verticals` | Lists the above | — |

## Install

### 1. From a release

1. Download the ZIP for your platform from [Releases](https://github.com/mouseos/Yahoo-Japan-Search-MCP/releases) and extract it
2. Run `yahoo-search-mcp-launcher` and click **Install** for your agent
3. Restart the agent

The launcher detects and registers Claude Desktop, Claude Code, Google Antigravity and Codex CLI.

### 2. From the command line

```bash
# print copy-paste config
yahoo-search-mcp setup

# Claude Code
claude mcp add Yahoo-Japan-Search-MCP -- /path/to/yahoo-search-mcp
```

### 3. Manual config

```json
{
  "mcpServers": {
    "Yahoo-Japan-Search-MCP": {
      "command": "/path/to/yahoo-search-mcp",
      "args": []
    }
  }
}
```

Leave `env` empty — there is nothing to configure.

### 4. Build from source

```bash
git clone https://github.com/mouseos/Yahoo-Japan-Search-MCP.git
cd Yahoo-Japan-Search-MCP
cargo build --release
# → target/release/yahoo-search-mcp
```

Requires Rust 1.80+ (for `std::sync::LazyLock`). On Linux the launcher additionally
needs `libfontconfig1-dev libxkbcommon-dev libgtk-3-dev`; the server itself has no
system dependencies.

## Use as a Rust library

The search logic lives in the provider crate (package `yahoo-search-mcp-provider`,
imported under its lib name `yahoo_search`), so you can call it directly from
your own Rust program without the MCP server. No API key, login or
configuration — anonymous, exactly like the MCP server.

```toml
[dependencies]
# From GitHub
yahoo_search = { git = "https://github.com/mouseos/Yahoo-Japan-Search-MCP", package = "yahoo-search-mcp-provider" }
# From a local checkout
# yahoo_search = { path = "../Yahoo-Japan-Search-MCP/crates/providers/yahoo_search", package = "yahoo-search-mcp-provider" }
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

```rust
use yahoo_search::YahooSearchClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = YahooSearchClient::new();
    let result = client.web_search("味噌ラーメン 作り方", 1, 5, "", "").await?;
    for item in &result.items {
        println!("{} — {}", item.title, item.url);
    }
    let suggest = client.suggest_keywords("ラーメン", 10).await?;
    for s in &suggest.suggestions {
        println!("{}", s.keyword);
    }
    Ok(())
}
```

Public API (everything is importable from the crate root):

| Kind | Names |
|---|---|
| Client | `YahooSearchClient` (`web_search` / `image_search` / `video_search` / `news_search` / `realtime_search` / `chiebukuro_search` / `suggest_keywords`, all returning `anyhow::Result`) |
| Shared result types | `SearchResult<T>`, `PROVIDER`, `verticals()` |
| Per-vertical item types | `WebItem`, `ImageItem` / `ImageSize`, `VideoItem`, `NewsItem`, `RealtimeItem`, `ChiebukuroItem`, `SuggestResult` / `Suggestion` |
| Offline helpers | `extract_next_data` / `extract_preloaded_state` / `extract_window_props` (payload extraction from saved page HTML), `clean_text`, `decode_entities`, `urlencode` |

Result types derive serde `Serialize` / `Deserialize`, so results can be
stored as JSON and read back. A network-free example lives in
`crates/providers/yahoo_search/examples/offline_basics.rs`, and
`crates/providers/yahoo_search/tests/library_api.rs` exercises the API
exactly as an external user would.

When a search fails through the MCP server, the tool result is returned as
`{provider, ok:false, error:{kind, message}}` (the form shared with
Baidu_MCP). Argument errors (such as a missing query) still surface as MCP
`isError` results, as before.

## Examples

Things to ask an agent:

- "Look up the Japanese tax filing deadline on the NTA site" → `yahoo_web_search` with `site: "nta.go.jp"`
- "What's trending in Japan right now?" → `yahoo_realtime_search`
- "Five recent news articles about the Bank of Japan" → `yahoo_news_search`
- "Search Chiebukuro for what to do when Windows won't boot" → `yahoo_chiebukuro_search`

Tool call:

```json
{
  "name": "yahoo_web_search",
  "arguments": { "query": "味噌ラーメン 作り方", "limit": 5, "updated": "month" }
}
```

Response (abridged):

```json
{
  "provider": "yahoo_japan",
  "vertical": "web",
  "ok": true,
  "query": "味噌ラーメン 作り方",
  "page": 1,
  "count": 5,
  "total_hits": 4210000,
  "next_page": 2,
  "related_queries": ["味噌ラーメン 作り方 簡単", "味噌ラーメン もやし 作り方"],
  "items": [
    {
      "rank": 1,
      "title": "濃厚！自家製！ 味噌ラーメンのレシピ動画・作り方",
      "url": "https://delishkitchen.tv/recipes/150781916893675939",
      "description": "手順 · 1. キャベツは食べやすい大きさに切る。…",
      "site": "delishkitchen.tv/recipes/150781916893675939"
    }
  ]
}
```

Japanese queries return substantially better results than English ones.

## Design

```
crates/
├── providers/yahoo_search/   # search logic
│   ├── common.rs             #   SSR payload extraction, HTML cleanup
│   ├── web.rs image.rs video.rs news.rs realtime.rs chiebukuro.rs suggest.rs
│   └── lib.rs                #   HTTP client and shared response types
├── server/                   # the MCP server itself (stdio JSON-RPC 2.0)
│   ├── main.rs               #   protocol handling
│   └── tools.rs              #   tool definitions and dispatch
└── launcher/                 # one-click registration GUI (egui)
```

Each vertical is read from the server-rendered payload the public search pages already
ship to browsers (`__NEXT_DATA__`, `__PRELOADED_STATE__`, `window.PROPS`) — structured
JSON rather than scraped DOM, so cosmetic markup changes do not break it.

### About the "appid"

The suggest backend is the one endpoint that wants an `appid`. It is not a user secret:
Yahoo embeds it verbatim in the HTML of its own public search page, which is where the
browser's search box reads it from. This server does the same — scrape on first use,
cache for the process lifetime, fall back to a known-good value. Nothing for you to supply.

## Development

```bash
cargo test --workspace                  # everything, including live tests against Yahoo
cargo test --workspace -- --skip e2e_   # offline tests only
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

The `e2e_*` tests hit the live Yahoo! JAPAN endpoints and verify every vertical still
parses. CI runs them daily so a change on Yahoo's side surfaces before users hit it.

Manual check over stdio:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"x","version":"1"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"yahoo_web_search","arguments":{"query":"ラーメン","limit":2}}}' \
  | ./target/debug/yahoo-search-mcp
```

## Notes

- Reads publicly served search result pages. Please use at a reasonable request rate.
- If Yahoo changes a page's structure, the affected vertical may fail; the error message says so explicitly.
- Not affiliated with, endorsed by, or connected to Yahoo Japan Corporation.

## License

MIT — see [LICENSE](LICENSE).
