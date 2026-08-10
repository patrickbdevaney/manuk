//! **G_MONOSPACE_BASE_SIZE — the monospace default size follows the computed FAMILY, not five tag
//! names.**
//!
//! `font-size: medium` — the initial value — resolves against a **per-generic** base size: 16px for
//! the variable-width default and **13px when the computed generic family is monospace**. That is
//! why `<code>` famously renders smaller than the prose around it.
//!
//! The engine wrote that rule as a UA **declaration**, `pre, code, kbd, samp, tt
//! { font-size: 13px }`, and a declaration says something different from the rule. Chrome, asked to
//! recite it rather than recalled:
//!
//! ```text
//!                                                      Chrome         ours, before
//!   <code> in  body { font: 16px monospace }         16px  38.53      13px  31   TOO SMALL
//!   <code> in  div  { font-size: 20px }              20px  48.17      13px  31   TOO SMALL
//!   <code> at the default size                       13px  31.31      13px  31   control ✓
//!   <span style="font-family:monospace"> default     13px  31.31      16px  39   TOO BIG
//! ```
//!
//! **Wrong in both directions at once.** A UA declaration beats inheritance by construction, so it
//! pinned every `<code>` and `<pre>` to 13px across the majority of the web that sets a body
//! font-size — documentation, wikis, blogs, every site with a design system — while *missing* the
//! element that actually asks for monospace. The tag list is a **constant fitted at one point**: it
//! agrees with Chrome on exactly the row where nobody has set a font size.
//!
//! The fix is the hook Stylo already calls from `font-size: medium`'s own computation
//! (`Device::base_size_for_generic`), which is also the only place that knows whether the size is
//! *still* `medium`. Measured on `doc.rust-lang.org/book`: shape **0.791 → 0.878** on an identical
//! 713-element sample.
//!
//! **The two negative rows come FIRST**, and they are what make this a gate rather than a
//! restatement: the default-size control passed before the fix too, and the `<span>` row is the one
//! the tag-keyed rule could not see.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<!-- NEGATIVE ROW 1: nobody sets a font-size. This is the ONLY row the old tag-keyed rule got
     right, and it must not move. -->
<div id="dflt"><code id="c_dflt">MMMM</code><span id="s_mono" style="font-family:monospace">MMMM</span><span id="s_prose">MMMM</span></div>
<!-- The inherited-size rows: an author font-size must reach the monospace element. -->
<div id="b16" style="font:16px/1.2 monospace"><code id="c16">MMMM</code><span id="s16">MMMM</span></div>
<div id="b20" style="font-size:20px"><code id="c20">MMMM</code><span id="s20" style="font-family:monospace">MMMM</span></div>
<script>
  var R = [];
  function q(id, p){ return getComputedStyle(document.getElementById(id))[p]; }
  function push(k, v){ R.push(k + '=' + v); }

  // ── NEGATIVE ROWS FIRST.
  push('dflt-code',  q('c_dflt','fontSize'));   // 13px — the monospace default, unchanged
  push('dflt-span',  q('s_mono','fontSize'));   // 13px — a SPAN that asks for monospace gets it too
  push('dflt-prose', q('s_prose','fontSize'));  // 16px — the variable-width default is untouched

  // ── The rule the tag-keyed declaration broke: an inherited size reaches the monospace element.
  push('inh16-code', q('c16','fontSize'));
  push('inh16-span', q('s16','fontSize'));
  push('inh20-code', q('c20','fontSize'));
  push('inh20-span', q('s20','fontSize'));

  document.getElementById('out').textContent = R.join('|');
</script></body></html>"##;

#[test]
fn the_monospace_default_size_follows_the_family_not_the_tag() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://mono.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        // ── NEGATIVE ROWS. The old tag-keyed rule passed the first and failed the second, which is
        //    exactly the discrimination this gate exists to make.
        (
            "dflt-code=13px",
            "THE CONTROL: with no author font-size, `<code>` is 13px — the monospace default. The \
             rule this replaces got this row right, so a gate that only asserted it would be green \
             against the bug",
        ),
        (
            "dflt-span=13px",
            "AND THE ROW THE TAG LIST COULD NOT SEE: a plain `<span style=\"font-family:monospace\">` \
             is ALSO 13px in Chrome. The default size belongs to the computed FAMILY; keying it on \
             five tag names gave this element 16px",
        ),
        (
            "dflt-prose=16px",
            "the variable-width default is untouched — a base size per generic, not one constant \
             swapped for another",
        ),
        // ── The inherited-size rows. A UA declaration beats inheritance, which is why the old rule
        //    could not express this at all.
        (
            "inh16-code=16px",
            "THE DEFECT: inside `body { font: 16px monospace }` Chrome gives `<code>` 16px. The UA \
             declaration pinned it to 13px — on the majority of the web, which sets a body font-size",
        ),
        (
            "inh16-span=16px",
            "its prose sibling agrees, so the divergence is the monospace element's and not the \
             block's",
        ),
        (
            "inh20-code=20px",
            "and it scales: 20px in, 20px out. A single inherited size would be a coincidence; two \
             is the rule",
        ),
        (
            "inh20-span=20px",
            "the `<span>` that asks for monospace inherits the author size too — `medium` is what \
             the 13px default replaces, and 20px is not `medium`",
        ),
    ] {
        assert!(
            got.split('|').any(|t| t == claim),
            "{claim}\n  {why}\n  got: {got}"
        );
    }
}
