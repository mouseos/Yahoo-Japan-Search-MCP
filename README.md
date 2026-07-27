# Yahoo! JAPAN Search MCP

Yahoo! JAPAN の検索をAIエージェントから使えるようにするMCPサーバー。**APIキー不要・アカウント不要・設定不要**。

[English](README.en.md)

[![CI](https://github.com/mouseos/Yahoo-Japan-Search-MCP/actions/workflows/ci.yml/badge.svg)](https://github.com/mouseos/Yahoo-Japan-Search-MCP/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## 対応する検索

| ツール | 検索 | 1ページあたり |
|---|---|---|
| `yahoo_web_search` | ウェブ検索 | 10 |
| `yahoo_image_search` | 画像検索 | 20 |
| `yahoo_video_search` | 動画検索 | 20 |
| `yahoo_news_search` | Yahoo!ニュース | 最大60（ページングなし）|
| `yahoo_realtime_search` | リアルタイム検索（X/Twitter） | 40 |
| `yahoo_chiebukuro_search` | Yahoo!知恵袋 | 10 |
| `yahoo_suggest_keywords` | サジェスト（キーワード補完） | 20 |
| `yahoo_list_verticals` | 上記の一覧を返す | — |

## インストール

### 1. リリースから

1. [Releases](https://github.com/mouseos/Yahoo-Japan-Search-MCP/releases) から自分のOS向けZIPをダウンロードして展開
2. `yahoo-search-mcp-launcher` を起動し、使いたいエージェントの **インストール** をクリック
3. エージェントを再起動

ランチャーは Claude Desktop / Claude Code / Google Antigravity / Codex CLI を自動検出して登録します。

### 2. コマンドラインから

```bash
# 設定用のJSONとコマンドを表示
yahoo-search-mcp setup

# Claude Code
claude mcp add Yahoo-Japan-Search-MCP -- /path/to/yahoo-search-mcp
```

### 3. 手動設定

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

`env` は空のままで構いません。設定する項目はありません。

### 4. ソースからビルド

```bash
git clone https://github.com/mouseos/Yahoo-Japan-Search-MCP.git
cd Yahoo-Japan-Search-MCP
cargo build --release
# → target/release/yahoo-search-mcp
```

Rust 1.80以上が必要です（`std::sync::LazyLock` のため）。Linuxではランチャーのビルドに
`libfontconfig1-dev libxkbcommon-dev libgtk-3-dev` が追加で必要です。サーバー本体に
システム依存はありません。

## 使用例

エージェントへの指示例:

- 「東京の確定申告の期限を国税庁のサイトで調べて」 → `yahoo_web_search`（`site: "nta.go.jp"`）
- 「今日本で何が話題になってる？」 → `yahoo_realtime_search`
- 「日銀の最新ニュースを5件」 → `yahoo_news_search`
- 「Windowsが起動しないときの対処法を知恵袋で」 → `yahoo_chiebukuro_search`

ツール呼び出し:

```json
{
  "name": "yahoo_web_search",
  "arguments": { "query": "味噌ラーメン 作り方", "limit": 5, "updated": "month" }
}
```

レスポンス（抜粋）:

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

日本語のクエリの方が英語より遥かに良い結果が返ります。

## 設計

```
crates/
├── providers/yahoo_search/   # 検索ロジック
│   ├── common.rs             #   SSRペイロード抽出・HTML整形
│   ├── web.rs image.rs video.rs news.rs realtime.rs chiebukuro.rs suggest.rs
│   └── lib.rs                #   HTTPクライアントと共通レスポンス型
├── server/                   # MCPサーバー本体（stdio JSON-RPC 2.0）
│   ├── main.rs               #   プロトコル処理
│   └── tools.rs              #   ツール定義とディスパッチ
└── launcher/                 # 登録用GUI（egui）
```

各検索は、Yahoo が既にブラウザへ配信しているサーバーレンダリング済みのペイロードから読み取ります
(`__NEXT_DATA__` / `__PRELOADED_STATE__` / `window.PROPS`)。HTMLのDOM構造ではなくJSONを読むため、
見た目のマークアップ変更には影響されません。

### APIキーについて

サジェストのバックエンドだけは `appid` を要求します。これはユーザーの秘密情報ではなく、
Yahoo が自社の公開検索ページのHTMLにそのまま埋め込んでいる値です（ブラウザの検索ボックスも
そこから読んでいます）。本サーバーも同じことをします — 初回利用時にスクレイプしてプロセス内で
キャッシュし、取得できなければ既知の値にフォールバックします。ユーザーが用意するものは何もありません。

## 開発

```bash
cargo test --workspace                  # 全テスト（実際にYahooへ通信するE2E含む）
cargo test --workspace -- --skip e2e_   # ネットワーク不要のテストのみ
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

`e2e_*` テストは実際の Yahoo! JAPAN に接続し、各検索が正しく解析できているかを検証します。
CIでは毎日実行され、Yahoo 側の変更を早期に検出します。

stdio越しの手動確認:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"x","version":"1"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"yahoo_web_search","arguments":{"query":"ラーメン","limit":2}}}' \
  | ./target/debug/yahoo-search-mcp
```

## 注意

- 公開されている検索結果ページを読み取ります。常識的な頻度で利用してください。
- Yahoo がページ構造を変更した場合、該当する検索が一時的に失敗することがあります。エラーメッセージにその旨が示されます。
- 本プロジェクトは Yahoo Japan Corporation とは無関係です。

## ライセンス

MIT — [LICENSE](LICENSE) を参照。
