//! G_NOSCRIPT_NOT_IN_EXTRACT — with scripting on, `<noscript>` content is raw text that is not
//! rendered, so it must not reach the observation text (Wolfram|Alpha's "doesn't run without
//! JavaScript" notice leaked into `extract` on a page whose JS ran fine). Fixture:
//! `docs/sprint/ext-run-a/noscript-fixture.html`.

use manuk_agent::AgentBrowser;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn noscript_text_is_not_extracted_when_scripting_runs() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/sprint/ext-run-a/noscript-fixture.html"
    );
    let mut b = AgentBrowser::new(800, 600);
    b.navigate(&format!("file://{path}")).await.unwrap();
    let text = b.observe().unwrap().text;
    assert!(text.contains("js-ran"), "script did not run: {text:?}");
    assert!(
        !text.contains("NOSCRIPT-NOTICE"),
        "noscript content leaked into the observation: {text:?}"
    );
}
