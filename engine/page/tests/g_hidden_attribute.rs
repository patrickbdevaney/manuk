//! **G_HIDDEN_ATTRIBUTE — the global `hidden` attribute collapses the element (and script can reveal it).**
//!
//! `<div hidden>` is one of the most common visibility toggles on the whole web: the pre-JS state of a
//! tab panel, the initial-collapsed body of an accordion, a feature-detect fallback shown only when a
//! script decides to, and the target of the idiomatic `el.hidden = false` / `toggleAttribute('hidden')`
//! show/hide. Per the HTML rendering spec (#hidden-elements) an element carrying the boolean `hidden`
//! attribute is **not rendered** — the UA sheet gives it `display: none`.
//!
//! It was measured **missing**: only `input[type=hidden]` was in the sheet, never the *global*
//! attribute, so `<div hidden>` reported `display: block` and painted its contents into the middle of
//! the page. Every "hidden until the script shows it" panel rendered permanently visible — the same
//! class of bug as a closed `<dialog>`/`<details>`/`[popover]` rendering its contents inline.
//!
//! There are three things to prove, and the third is the one that makes it a live toggle rather than a
//! static rule:
//!
//! 1. `<div hidden>` is `display: none` — it does not render.
//! 2. `hidden="until-found"` is the spec exception: it is rendered with `content-visibility: hidden`
//!    (collapsed-but-findable), which we do not support yet, so it must be left **visible** rather than
//!    falsely collapsed into content a user could never reveal. It must NOT be `display: none`.
//! 3. Removing the attribute from script (`el.hidden = false`) re-renders it — the cascade re-runs on
//!    the attribute mutation, so the selector stops matching and the element comes back. A rule that
//!    could not be undone would break every toggle it is meant to serve.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="plain" hidden>plain hidden — must NOT render</div>
<div id="uf" hidden="until-found">until-found — left visible (no content-visibility yet)</div>
<div id="reveal" hidden>revealed by script</div>
<div id="shown">a normal visible element</div>
<div id="out">-</div>
<script>
  var R = [];
  R.push('plain:' + getComputedStyle(document.getElementById('plain')).display);
  R.push('uf:' + getComputedStyle(document.getElementById('uf')).display);
  // The idiomatic reveal: flip the reflected boolean property off.
  document.getElementById('reveal').hidden = false;
  R.push('revealed:' + getComputedStyle(document.getElementById('reveal')).display);
  R.push('shown:' + getComputedStyle(document.getElementById('shown')).display);
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn a_global_hidden_attribute_collapses_and_script_can_reveal_it() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://hidden.test/", &fonts, 400.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "plain:none",
            "a `<div hidden>` must NOT render. Only `input[type=hidden]` was in the UA sheet, so the \
             global boolean attribute reported `display:block` and painted its contents into the page",
        ),
        (
            "uf:block",
            "`hidden=\"until-found\"` is the spec exception — rendered with `content-visibility: hidden`, \
             which we do not support yet, so it must be left visible rather than falsely collapsed into \
             content a user could never reveal on find. It must NOT be `display:none`",
        ),
        (
            "revealed:block",
            "removing the attribute from script (`el.hidden = false`) must re-render the element — the \
             cascade re-runs on the mutation and the `[hidden]` selector stops matching. A rule that \
             could not be undone would break every toggle it is meant to serve",
        ),
        (
            "shown:block",
            "a normal sibling with no `hidden` attribute is unaffected — the rule is attribute-scoped, \
             not a blanket collapse",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_HIDDEN_ATTRIBUTE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
