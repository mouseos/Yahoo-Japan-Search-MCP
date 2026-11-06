//! Offline basics for using `yahoo_search` as a library — no network.
//!
//! Shows the parts of the public API that work without contacting Yahoo:
//! building result values, serialising them, listing the verticals, and
//! extracting the server-rendered payload from a saved search page.
//!
//! Run with:
//! `cargo run -p yahoo-search-mcp-provider --example offline_basics`
//!
//! The network searches themselves are one-liners on the same client, e.g.
//! `client.web_search("味噌ラーメン 作り方", 1, 5, "", "").await?` inside an
//! async (tokio) context — see the crate-level documentation.

use yahoo_search::{
    clean_text, extract_next_data, urlencode, verticals, SearchResult, WebItem, YahooSearchClient,
    PROVIDER,
};

fn main() {
    // The client is cheap to construct and needs no configuration; creating
    // it performs no network I/O.
    let _client = YahooSearchClient::new();

    // Build a result value exactly as a search would return it, and
    // serialise it (result types derive serde::Serialize/Deserialize).
    let item = WebItem {
        rank: 1,
        title: clean_text("<b>濃厚</b>！味噌ラーメンの作り方"),
        url: "https://example.jp/recipes/1".to_string(),
        description: Some("手順を写真付きで紹介".to_string()),
        site: Some("example.jp".to_string()),
    };
    let result = SearchResult::new("web", "味噌ラーメン 作り方", 1, vec![item])
        .with_total(Some(4_210_000))
        .with_next_page(Some(2));
    assert_eq!(result.provider, PROVIDER);
    println!("provider={} count={}", result.provider, result.count);
    println!("{}", serde_json::to_string_pretty(&result).unwrap());

    // Extract the `__NEXT_DATA__` payload from a saved search page.
    let saved_html = r#"<html><head></head><body>
<script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{}}}</script>
</body></html>"#;
    let payload = extract_next_data(saved_html).expect("payload should parse");
    println!("payload props present: {}", payload["props"].is_object());

    // Query-string encoding, as used for every search URL.
    println!("urlencode: {}", urlencode("味噌ラーメン 作り方"));

    // Machine-readable list of the searchable verticals.
    let v = verticals();
    let ids: Vec<&str> = v["verticals"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .collect();
    println!("verticals: {}", ids.join(", "));
}
