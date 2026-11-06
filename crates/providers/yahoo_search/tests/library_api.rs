//! External-use tests: this file deliberately imports *only* from the crate
//! root (`use yahoo_search::{...}`), exactly as a downstream library user
//! would. No per-vertical module paths, no internal helpers.
//!
//! Everything here is offline: constructing values, serialising results,
//! extracting payloads from saved HTML and checking that the async search
//! methods exist with the documented signatures (by taking references to
//! them — calling them would hit the network).

use yahoo_search::{
    clean_text, decode_entities, extract_next_data, extract_preloaded_state, extract_window_props,
    urlencode, verticals, ChiebukuroItem, ImageItem, ImageSize, NewsItem, RealtimeItem,
    SearchResult, SuggestResult, Suggestion, VideoItem, WebItem, YahooSearchClient, PROVIDER,
};

#[test]
fn provider_constant_and_client_construction() {
    assert_eq!(PROVIDER, "yahoo_japan");
    // Construction performs no network I/O and needs no configuration.
    let _client = YahooSearchClient::new();
    let _default = YahooSearchClient::default();
}

#[test]
fn search_methods_are_reachable_from_the_root_client_type() {
    // Referencing each method as a function item type-checks its public
    // signature without performing any request.
    let _web = YahooSearchClient::web_search;
    let _image = YahooSearchClient::image_search;
    let _video = YahooSearchClient::video_search;
    let _news = YahooSearchClient::news_search;
    let _realtime = YahooSearchClient::realtime_search;
    let _chiebukuro = YahooSearchClient::chiebukuro_search;
    let _suggest = YahooSearchClient::suggest_keywords;
}

#[test]
fn web_result_builds_and_serialises() {
    let item = WebItem {
        rank: 1,
        title: "タイトル".to_string(),
        url: "https://example.jp/".to_string(),
        description: Some("説明".to_string()),
        site: None,
    };
    let result = SearchResult::new("web", "クエリ", 1, vec![item])
        .with_total(Some(123))
        .with_next_page(Some(2))
        .with_related(vec!["関連 クエリ".to_string()])
        .with_note("note");
    assert!(result.ok);
    assert_eq!(result.vertical, "web");
    assert_eq!(result.count, 1);
    assert_eq!(result.items[0].title, "タイトル");

    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["provider"], "yahoo_japan");
    assert_eq!(json["total_hits"], 123);
    assert_eq!(json["items"][0]["url"], "https://example.jp/");

    let back: SearchResult<WebItem> = serde_json::from_value(json).unwrap();
    assert_eq!(back.items[0].url, "https://example.jp/");
}

#[test]
fn roundtrip_tolerates_omitted_empty_vecs() {
    // Empty Vec fields are skipped when serialising; deserialising must
    // still succeed (serde `default`). Regression test: this round-trip
    // used to fail with "missing field `related_queries`".
    let result: SearchResult<NewsItem> = SearchResult::new(
        "news",
        "クエリ",
        1,
        vec![NewsItem {
            id: "1".to_string(),
            title: "見出し".to_string(),
            url: "https://news.yahoo.co.jp/articles/1".to_string(),
            publisher: None,
            published_at: None,
            summary: None,
            categories: vec![],
            thumbnail: None,
            article_type: None,
        }],
    );
    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains("related_queries"));
    assert!(!json.contains("categories"));
    let back: SearchResult<NewsItem> = serde_json::from_str(&json).unwrap();
    assert!(back.related_queries.is_empty());
    assert!(back.items[0].categories.is_empty());
}

#[test]
fn every_vertical_item_type_is_constructible_from_the_root() {
    let _image = ImageItem {
        id: "1".to_string(),
        title: "img".to_string(),
        source_url: None,
        source_site: None,
        original: Some(ImageSize {
            url: "https://example.jp/a.jpg".to_string(),
            width: Some(640),
            height: Some(480),
        }),
        cached: None,
        thumbnail: None,
        file_format: Some("jpg".to_string()),
    };
    let _video = VideoItem {
        id: "1".to_string(),
        title: "movie".to_string(),
        url: "https://example.jp/v".to_string(),
        source: None,
        uploader: None,
        duration: Some("12:34".to_string()),
        upload_date: None,
        summary: None,
        thumbnail: None,
    };
    let _news = NewsItem {
        id: "1".to_string(),
        title: "見出し".to_string(),
        url: "https://news.yahoo.co.jp/articles/1".to_string(),
        publisher: Some("配信社".to_string()),
        published_at: None,
        summary: None,
        categories: vec![],
        thumbnail: None,
        article_type: None,
    };
    let _realtime = RealtimeItem {
        id: "1".to_string(),
        url: "https://x.com/u/status/1".to_string(),
        text: "本文".to_string(),
        author_name: "名前".to_string(),
        author_handle: "handle".to_string(),
        author_url: None,
        created_at: Some(1_700_000_000),
        reply_count: 0,
        repost_count: 0,
        like_count: 0,
        hashtags: vec![],
        media: vec![],
    };
    let _chiebukuro = ChiebukuroItem {
        id: "q1".to_string(),
        title: "質問".to_string(),
        url: "https://detail.chiebukuro.yahoo.co.jp/qa/question_detail/q1".to_string(),
        summary: None,
        status: Some("solved".to_string()),
        posted_at: None,
        updated_at: None,
        answer_count: 3,
        view_count: 42,
        categories: vec![],
    };
    let suggest = SuggestResult {
        provider: PROVIDER.to_string(),
        vertical: "suggest".to_string(),
        ok: true,
        query: "ラーメン".to_string(),
        count: 1,
        suggestions: vec![Suggestion {
            keyword: "ラーメン 作り方".to_string(),
            search_url: "https://search.yahoo.co.jp/search?p=x".to_string(),
        }],
    };
    assert_eq!(suggest.suggestions[0].keyword, "ラーメン 作り方");
}

#[test]
fn offline_payload_extraction_from_saved_pages() {
    let next = extract_next_data(
        r#"<script id="__NEXT_DATA__" type="application/json">{"props":{}}</script>"#,
    )
    .unwrap();
    assert!(next["props"].is_object());

    let state =
        extract_preloaded_state("<script>__PRELOADED_STATE__ = {\"search\":{}};</script>").unwrap();
    assert!(state["search"].is_object());

    let props =
        extract_window_props("<script>window.PROPS = {\"pagination\":{}};window.x = 1;</script>")
            .unwrap();
    assert!(props["pagination"].is_object());

    assert!(extract_next_data("<html>no payload</html>").is_err());
}

#[test]
fn text_helpers_are_root_level() {
    assert_eq!(clean_text("<b>ラーメン</b> &amp; 味噌"), "ラーメン & 味噌");
    assert_eq!(decode_entities("a &lt; b"), "a < b");
    assert_eq!(urlencode("a b&c"), "a%20b%26c");
}

#[test]
fn verticals_listing_covers_all_searches() {
    let v = verticals();
    assert_eq!(v["provider"], PROVIDER);
    assert_eq!(v["auth"], "none");
    let ids: Vec<&str> = v["verticals"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "web",
            "image",
            "video",
            "news",
            "realtime",
            "chiebukuro",
            "suggest"
        ]
    );
}
