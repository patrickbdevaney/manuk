//! **G_PSEUDO_INDEX_BUCKETS — narrowing the pseudo candidate set must not drop a rule.**
//!
//! `PseudoIndex` hoisted the *collection* of `::before`/`::after` rules out of the per-element loop
//! and left the *matching* as a linear scan of every collected rule for every element — the exact
//! `O(elements × rules)` shape `RuleIndex`'s tag/class/id bucketing had already been built to fix,
//! one struct away in the same file. Measured on `bhramarah.in` (23,001 elements, 51 sheets) with
//! `MANUK_CASCADE_PROFILE=1`: pseudo matching was **43% of an 8,208 ms cascade — 3,531 ms, twice
//! what matching every ordinary selector cost.**
//!
//! t1239 gives `PseudoIndex` the same buckets. **The danger is not slowness, it is silence:** a
//! selector whose bucket key is computed wrongly lands in a bucket the element never consults, and
//! the rule simply never applies — no error, no warning, one missing icon. So this gate asserts
//! BEHAVIOUR across every bucket a `::before` selector can land in, plus the two that must fall
//! through to the universal bucket because they have no cheap key at all.
//!
//! **How to break it:** make `bucket_key_of` return a key for `[data-x]::before` (it must return
//! `None` → universal), or bucket on any compound other than the rightmost — `.wrap .item::before`
//! keys on `item`, and keying on `wrap` puts it where `.item` will never look.
//!
//! ⚠ **RED-proven by emptying the candidate set** (`picked.clear()` after `index.candidates`): all
//! nine claims fail, so the narrowing is load-bearing and this gate is looking at it. Stated
//! precisely because the per-bucket RED patches were *not* each proven separately — a first attempt
//! at one (inventing a key for attribute selectors) turned out to be a silent no-op, which is its
//! own recurring lesson here: **a mutation that does not go red may be a mutation that did not
//! apply.**

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  /* one rule per BUCKET the key function can choose, all on distinct elements */
  em::before          { content: "TAG-"; }        /* tag bucket   */
  .cls::before        { content: "CLASS-"; }      /* class bucket */
  #uid::before        { content: "ID-"; }         /* id bucket    */
  *::before           { content: "STAR-"; }       /* universal    */
  [data-k]::before    { content: "ATTR-"; }       /* NO key -> must fall to universal */
  .wrap .deep::before { content: "DESC-"; }       /* keys on the RIGHTMOST compound   */
  li:nth-child(2)::before { content: "NTH-"; }    /* tag bucket, positional           */
  b::after            { content: "-AFT"; }        /* the ::after index, separately     */
</style></head><body>
<div id="out">-</div>
<em id="tagEl"></em>
<span class="cls"></span>
<span id="uid"></span>
<i id="starEl"></i>
<span data-k="1" id="attrEl"></span>
<div class="wrap"><span class="deep" id="deepEl"></span></div>
<ul><li id="li1"></li><li id="li2"></li></ul>
<b id="aftEl"></b>
<script>
  var R = [], g = function (id, p) {
    return getComputedStyle(document.getElementById(id), p || '::before').content;
  };
  R.push('tag:' + g('tagEl'));
  R.push('cls:' + g('cls0'));
  R.push('id:' + g('uid'));
  R.push('star:' + g('starEl'));
  R.push('attr:' + g('attrEl'));
  R.push('desc:' + g('deepEl'));
  R.push('nth1:' + g('li1'));
  R.push('nth2:' + g('li2'));
  R.push('aft:' + g('aftEl', '::after'));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn every_pseudo_bucket_still_applies_its_rule() {
    let fonts = FontContext::new();
    // The `.cls` element needs an id for the script; give it one without touching the selector
    // under test (the rule keys on the CLASS, and the id must not change which bucket it lands in).
    let html = HTML.replace(
        r#"<span class="cls"></span>"#,
        r#"<span class="cls" id="cls0"></span>"#,
    );
    let page = manuk_page::Page::load(&html, "https://pseudo.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("tag:\"TAG-\"", "a TAG-keyed ::before rule must reach an <em>"),
        (
            "cls:\"CLASS-\"",
            "a CLASS-keyed rule must reach the element carrying that class",
        ),
        (
            "id:\"ID-\"",
            "an ID-keyed rule must reach the element with that id",
        ),
        (
            "star:\"STAR-\"",
            "`*::before` has no cheap key and belongs in the UNIVERSAL bucket, which every element consults",
        ),
        (
            "attr:\"ATTR-\"",
            "`[data-k]::before` ALSO has no cheap key — an attribute is not a bucket, so it must fall through to universal. Inventing a key for it is the silent-drop failure this gate exists for",
        ),
        (
            "desc:\"DESC-\"",
            "`.wrap .deep::before` must key on the RIGHTMOST compound (`deep`). Keying on `wrap` files it where `.deep` never looks, and the rule vanishes with no error",
        ),
        (
            "nth2:\"NTH-\"",
            "`li:nth-child(2)::before` keys on the tag and is then fully matched — the positional part must still be evaluated",
        ),
        (
            "aft:\"-AFT\"",
            "the ::after index is a SEPARATE bucket set; a fix applied to ::before only would leave this one linear (or, worse, empty)",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_PSEUDO_INDEX_BUCKETS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }

    // The other polarity: narrowing must not make a rule apply where it did not. `li:nth-child(2)`
    // is in the same tag bucket as `li1`, so `li1` DOES consult it and must still lose on matching.
    assert!(
        got.contains("nth1:\"STAR-\""),
        "G_PSEUDO_INDEX_BUCKETS: the FIRST <li> must fall back to `*::before`, not pick up \
         `:nth-child(2)`.\n  got: {got}\n\n  Bucketing decides what to SKIP; every survivor is \
         still fully matched. A bucket that is treated as a match is the opposite failure and \
         would be invisible in a fixture where only one rule exists per element."
    );
}
