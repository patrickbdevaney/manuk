//! Live end-to-end web-font test (A3): fetch a real WOFF2 over the network, decode
//! it to sfnt via the pure-Rust reconstructor, and confirm it parses as a usable font.
//!
//! Gated behind `#[ignore]` because it needs outbound network — the offline unit test
//! over the committed `Ahem.woff2` fixture (in `manuk_text::woff2`) is the CI path.
//! Run explicitly with: `cargo test -p manuk-page --test webfont_live -- --ignored`.

/// A stable, CDN-hosted WOFF2 (Fontsource's Roboto 400) — real Google-Fonts output
/// with the glyf/loca and hmtx transforms applied.
const WOFF2_URL: &str =
    "https://cdn.jsdelivr.net/npm/@fontsource/roboto@5.0.8/files/roboto-latin-400-normal.woff2";

#[test]
#[ignore = "requires network"]
fn live_woff2_decodes_to_valid_sfnt() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let bytes = rt.block_on(async {
        let resp = manuk_net::fetch(WOFF2_URL).await.expect("fetch woff2");
        assert!(resp.status < 400, "http status {}", resp.status);
        resp.body.to_vec()
    });
    assert_eq!(&bytes[..4], b"wOF2", "server returned a WOFF2");

    let sfnt = manuk_text::decode_woff2(&bytes).expect("decode woff2 -> sfnt");
    // Reconstructed TrueType-flavored sfnt.
    assert_eq!(&sfnt[..4], &[0x00, 0x01, 0x00, 0x00]);

    // fontdb (ttf-parser) must parse it and read a family name.
    let mut db = fontdb::Database::new();
    db.load_font_data(sfnt);
    let face = db.faces().next().expect("one face");
    assert!(
        face.families.iter().any(|(n, _)| n.eq_ignore_ascii_case("roboto")),
        "expected Roboto family, got {:?}",
        face.families
    );
}
