//! **G_RADIO_HANDLES_RESOLVE_EXACTLY — `resolve_handle("Radio B")` must return Radio B.**
//!
//! t1402: the intent keywords drop every token shorter than two characters, so `Radio A`,
//! `Radio B` and `Radio C` all reduced to `["radio"]`, tied, and the first/arbitrary one won.
//! An exact (case-insensitive, whitespace-normalised) accessible-name match among the nodes of
//! the requested role now outranks the fuzzy scorer.
//!
//! Mutation that must turn this red: delete the exact-name pre-pass in `AgentBrowser::resolve`.

use manuk_a11y::Role;
use manuk_agent::AgentBrowser;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><title>R</title><style>
body{margin:0;font:16px/1.4 monospace} label{display:block;height:24px}
</style></head><body><form>
<label><input type=radio name=g value=a> Radio A</label>
<label><input type=radio name=g value=b> Radio B</label>
<label><input type=radio name=g value=c> Radio C</label>
<label><input type=radio name=g value=ab> Radio AB</label>
</form></body></html>"##;

#[test]
fn each_radio_resolves_to_itself() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let path = std::env::temp_dir().join("g_radio_handles.html");
    std::fs::write(&path, HTML).unwrap();
    let mut b = AgentBrowser::new(1024, 768);
    rt.block_on(async { b.navigate(&format!("file://{}", path.display())).await.unwrap() });
    let tree = b.a11y_tree().unwrap();
    let name_of = |id| tree.iter().find(|n| n.node == id).map(|n| n.name.trim().to_string());

    let mut seen = std::collections::HashSet::new();
    for want in ["Radio A", "Radio B", "Radio C", "Radio AB"] {
        let id = b.resolve_handle(&Role::Radio, want).expect(want);
        assert_eq!(name_of(id).as_deref(), Some(want), "resolve_handle({want:?}) returned the wrong radio");
        assert!(seen.insert(id.0), "{want} collided with an earlier radio");
    }
}
