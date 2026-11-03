//! **G_LIVE_NOW_TICKER_HAS_WIDTH — the whitehouse.gov live-bar script sizes its marquee with
//! `Math.ceil(1920 / ticker.offsetWidth)`; a ticker that reports 0 makes that loop unbounded.**
//! Fixture = the page's own live-bar DOM (built by script, prepended into `.site-header`) and the
//! theme's `.live-now*` rules. Expected values are headless Chrome 145.0.7632.116 on this fixture:
//! ticker;item;bar;pill = 195;195;764;76 at 800 px wide.
//!
//! ⚠ The handler runs in a **fetch delivery**, not at load: `resolve_fetch` ran the script round with
//! no forced-reflow hook, so a geometry read of an element the handler had just created answered
//! from the pre-script snapshot (no entry → 0). A page that sizes its marquee as
//! `Math.ceil(1920/offsetWidth)` then loops for ever.
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
:root{--g:1rem;--fs:1rem;--pfs:.375rem;--h:40px;--red:#c00;--white:#fff}
.live-now{align-items:center;background-color:var(--red);color:var(--white);display:flex;font-family:serif;font-size:var(--fs);gap:var(--g);height:var(--h);letter-spacing:-.0099897915em;line-height:normal;overflow:hidden;position:relative;text-decoration:none;text-transform:uppercase;user-select:none;z-index:10005}
.site-header:has(.wp-block-x) .live-now{display:flex}
.site-header:has(.wp-block-x){animation:none!important;display:contents}
.live-now__ticker{display:flex;flex-shrink:0;gap:var(--g);will-change:transform}
.live-now__item{align-items:center;display:flex;gap:var(--g);white-space:nowrap}
.live-now__pill{border:.1111917286em solid var(--white);border-radius:1.3899873027em;display:block;font-family:sans-serif;font-size:var(--pfs);letter-spacing:.4298929802em;line-height:normal;padding:.7255577725em 1.0883366588em;text-transform:uppercase}
</style></head><body><header class="site-header"><div class="wp-block-x">x</div></header><div id=out></div><script>
fetch('/live.json').then(r=>r.text()).then(function(){
const t=document.querySelector(".site-header");const i=document.createElement("a");i.href="/live/";i.classList.add("live-now");
const o=document.createElement("span");o.classList.add("live-now__ticker");const a=document.createElement("span");a.textContent="Live Stream";a.classList.add("live-now__item");
const c=document.createElement("span");c.textContent="Watch Now";c.classList.add("live-now__pill");a.appendChild(c);o.appendChild(a);i.appendChild(o);t.prepend(i);
document.getElementById('out').textContent=o.offsetWidth+';'+a.offsetWidth+';'+i.offsetWidth+';'+c.offsetWidth;
});
</script></body></html>
"##;

#[test]
fn the_live_now_ticker_has_a_nonzero_offset_width() {
    let fonts = FontContext::new();
    let mut p = manuk_page::Page::load(HTML, "http://x/", &fonts, 800.0);
    let reqs = p.take_fetches();
    assert_eq!(reqs.len(), 1, "the page issues one /live.json fetch");
    p.resolve_fetch(reqs[0].0, 200, "{}", &[], &fonts, 800.0);
    let root = p.dom().root();
    let hits = manuk_css::query_selector_all(p.dom(), root, "#out");
    assert!(!hits.is_empty(), "fixture is missing #out");
    let got = p.dom().text_content(hits[0]);
    assert!(
        got.contains(';'),
        "the fetch handler must run — got {got:?}"
    );
    let v: Vec<f64> = got.split(';').map(|x| x.parse().unwrap_or(-1.0)).collect();
    assert!(
        v[0] > 0.0,
        "ticker offsetWidth is 0 (Chrome: 195) — the page's loop would never end; got {got:?}"
    );
    assert!(
        (v[0] - 195.0).abs() <= 19.5,
        "ticker offsetWidth {} not within 10% of Chrome's 195; got {got:?}",
        v[0]
    );
}
