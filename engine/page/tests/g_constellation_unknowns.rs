//! **G_CONSTELLATION_UNKNOWNS — the 49 `unknown` rows of the capability map, MEASURED and PINNED.**
//!
//! Surface audit #81 (tick 1393). The audit before it (t1383) ranked *"the 49 `unknown` rows are the
//! frontier now, not missing rows"* as its #2 finding. Ten ticks later the number was **still exactly
//! 49** — the map was complete and un-measured, which is the one state the ratchet's MEASURED
//! invariant is built to punish and the one nothing was punishing.
//!
//! This is the instrument that closes them: one page, 58 observable questions, run through THIS engine
//! and through headless Chrome, and every answer pinned. It is a **measure-and-pin** gate, not a
//! conformance gate — most rows below assert what we DO NOT have. That is deliberate:
//!
//!   * an `UNDEF` from `getComputedStyle` is a MEASUREMENT (the property is not modelled), and pinning
//!     it means the day somebody models it, this gate goes red and the map gets updated with it —
//!     instead of the map quietly rotting into a lie the way it just did for ten ticks;
//!   * `CSS.supports` and `getComputedStyle` are TWO ENTRANCES (t1353) and both are asked, because a
//!     property can answer one and not the other;
//!   * a row that says `absent` is a capability claim we can be held to, which `unknown` never was.
//!
//! ⭐ **`contrast-color()` is the one row where WE say yes and CHROME says no, and we are right.**
//! `CSS.supports('color: contrast-color(red)')` is `true` here and `false` in Chrome, and the function
//! actually computes: black on a white backdrop, **white on a black one**, and it correctly overrides
//! an earlier `color: red` in the same rule. Chrome drops the declaration as invalid, so its `#c` keeps
//! `red` and ours goes black.
//!
//! ⚠⚠ **That divergence is CORRECT and must not be "fixed".** It is the North Star's exact case:
//! Chromium is the CEILING on capability, not the floor, and an oracle diff that sees `red` vs `black`
//! here is looking at us having MORE of an Interop 2026 focus area than the reference, not less. It is
//! recorded in this gate so a future diff cannot quietly reverse it.
//!
//! ⚠ ONE `#[test]` in this binary: two SpiderMonkey contexts in one manuk-page binary tear each other
//! down and the second test reads the first one's empty output.

use manuk_text::FontContext;

const HTML: &str = r####"<!doctype html><html><head><style>
#ff { font-feature-settings: "liga" 0, "kern" 1; }
#tds { text-decoration: underline; text-decoration-style: wavy; }
#tdt { text-decoration: underline; text-decoration-thickness: 3px; }
#tdsi { text-decoration: underline; text-decoration-skip-ink: all; }
#ad { animation: none; animation-delay: 2.5s; }
#op { offset-path: path("M 0 0 L 100 100"); offset-distance: 50%; offset-rotate: 45deg; }
#rc { color: rgb(from #ff0000 r g b / 50%); }
#icu { width: 10ic; }
#brk { break-before: page; break-inside: avoid; break-after: column; }
#gapd { display: grid; gap: 10px; column-rule-color: red; column-rule-style: solid; column-rule-width: 2px; }
#rg { background: radial-gradient(closest-corner at 30% 40%, red, blue); }
#cc { caret-color: rgb(0, 128, 0); }
#io { image-orientation: none; }
#tes { text-emphasis-style: dot; }
#po { paint-order: stroke fill; }
#bdb { box-decoration-break: clone; }
#anch { anchor-name: --a; }
#anp { position: absolute; position-area: top left; }
#ws { -webkit-text-stroke: 1px red; }
</style></head><body>
<span id="ff">fi</span><span id="tds">x</span><span id="tdt">x</span><span id="tdsi">x</span>
<div id="ad"></div><div id="op"></div><div id="rc"></div><div id="icu"></div><div id="brk"></div>
<div id="gapd"></div><div id="rg"></div><div id="cc"></div><img id="io"><span id="tes">x</span>
<span id="po">x</span><span id="bdb">x</span><div id="anch"></div><div id="anp"></div><span id="ws">x</span>
<select id="selm" multiple><option>a</option><option>b</option><option>c</option></select>
<select id="sels"><option>a</option><option>b</option><option>c</option></select>
<input id="minmax" minlength="2" maxlength="8">
<dialog id="dlg"></dialog>
<button id="cmd"></button>
<img id="sza" sizes="auto" srcset="x.png 100w" loading="lazy">
<div id="out">-</div>
<script>
var r=[];
function k(n,v){ r.push(n+'='+v); }
function cs(id,prop){ try{ var v=getComputedStyle(document.getElementById(id))[prop];
  return (v===undefined)?'UNDEF':(v===''?'EMPTY':String(v)); }catch(e){ return 'THROW'; } }
function has(o,p){ try{ return (p in o)?'yes':'no'; }catch(e){ return 'THROW'; } }
function typ(n){ try{ return (typeof window[n]==='undefined')?'absent':'present'; }catch(e){ return 'THROW'; } }

// ── CSS properties: does the declaration SURVIVE to computed style? ─────────
k('font-feature-settings',   cs('ff','fontFeatureSettings'));
k('text-decoration-style',   cs('tds','textDecorationStyle'));
k('text-decoration-thickness', cs('tdt','textDecorationThickness'));
k('text-decoration-skip-ink', cs('tdsi','textDecorationSkipInk'));
k('animation-delay',         cs('ad','animationDelay'));
k('offset-path',             cs('op','offsetPath'));
k('offset-distance',         cs('op','offsetDistance'));
k('offset-rotate',           cs('op','offsetRotate'));
k('relative-color',          cs('rc','color'));
k('ic-unit',                 cs('icu','width'));
k('break-before',            cs('brk','breakBefore'));
k('break-inside',            cs('brk','breakInside'));
k('column-rule-color',       cs('gapd','columnRuleColor'));
k('column-rule-style',       cs('gapd','columnRuleStyle'));
k('column-rule-width',       cs('gapd','columnRuleWidth'));
k('radial-closest-corner',   cs('rg','backgroundImage'));
k('caret-color',             cs('cc','caretColor'));
k('image-orientation',       cs('io','imageOrientation'));
k('text-emphasis-style',     cs('tes','textEmphasisStyle'));
k('paint-order',             cs('po','paintOrder'));
k('box-decoration-break',    cs('bdb','boxDecorationBreak'));
k('anchor-name',             cs('anch','anchorName'));
k('position-area',           cs('anp','positionArea'));

// ── CSS.supports, the OTHER entrance (t1353: check both doors) ─────────────
function sup(d){ try{ return CSS.supports(d)?'yes':'no'; }catch(e){ return 'THROW'; } }
k('sup:if()',            sup('color: if(style(--x): red; else: blue)'));
k('sup:contrast-color',  sup('color: contrast-color(red)'));
k('sup:shape()',         sup('clip-path: shape(from 0% 0%, line to 100% 0%)'));
k('sup:zoom',            sup('zoom: 2'));
k('sup:container-style', sup('container-name: --c'));
k('sup:scroll-marker',   sup('(selector(::scroll-marker))'));
k('sup:gap-decorations', sup('column-rule-color: red'));
k('sup:update-media',    (function(){ try{ return matchMedia('(update: fast)').matches!==undefined?'yes':'no'; }catch(e){ return 'THROW'; } })());
k('sup:device-width',    (function(){ try{ return matchMedia('(device-width: 800px)').media!=='not all'?'yes':'no'; }catch(e){ return 'THROW'; } })());

// ── API presence ───────────────────────────────────────────────────────────
k('PressureObserver',   typ('PressureObserver'));
k('AudioContext',       typ('AudioContext'));
k('Translator',         typ('Translator'));
k('LanguageDetector',   typ('LanguageDetector'));
k('Summarizer',         typ('Summarizer'));
k('serviceWorker',      (function(){ try{ return navigator.serviceWorker?'present':'absent'; }catch(e){ return 'THROW'; } })());
k('ClipboardEvent',     typ('ClipboardEvent'));
k('execCommand',        (function(){ try{ return (typeof document.execCommand==='function')?'present':'absent'; }catch(e){ return 'THROW'; } })());
k('queryCommandSupported-undo', (function(){ try{ return String(document.queryCommandSupported('undo')); }catch(e){ return 'THROW'; } })());

// ── HTML/IDL surfaces ──────────────────────────────────────────────────────
k('input.minLength',    (function(){ var e=document.getElementById('minmax'); return String(e.minLength)+'/'+String(e.maxLength); })());
k('dialog.closedBy',    has(document.getElementById('dlg'),'closedBy'));
k('button.command',     has(document.getElementById('cmd'),'command'));
k('button.commandForElement', has(document.getElementById('cmd'),'commandForElement'));
k('interestTargetElement', has(document.getElementById('cmd'),'interestTargetElement'));
k('writingSuggestions', has(document.getElementById('cmd'),'writingSuggestions'));
k('img.sizes-auto',     (function(){ var e=document.getElementById('sza'); return String(e.sizes); })());
k('CustomElementRegistry', typ('CustomElementRegistry'));
k('WebTransport',       typ('WebTransport'));
k('RTCPeerConnection',  typ('RTCPeerConnection'));
k('WebAssembly.Suspending', (function(){ try{ return (typeof WebAssembly!=='undefined' && typeof WebAssembly.Suspending!=='undefined')?'present':'absent'; }catch(e){ return 'THROW'; } })());
k('Highlight',          typ('Highlight'));
k('navigation',         (function(){ try{ return (typeof navigation!=='undefined')?'present':'absent'; }catch(e){ return 'absent'; } })());

// ── GEOMETRY: <select multiple> must be a LIST BOX, not a dropdown ─────────
k('select-multiple-taller', (function(){
  var m=document.getElementById('selm').getBoundingClientRect().height;
  var s=document.getElementById('sels').getBoundingClientRect().height;
  return (m > s + 4) ? 'yes' : 'no';   // the PIXELS differ by engine; the KIND of control does not
})());

// ── contrast-color(): the ONE row where WE say yes and Chrome says no ──────
(function(){
  var mk = function(css){ var d=document.createElement('span'); d.setAttribute('style', css);
                          document.body.appendChild(d); return d; };
  k('cc-on-white', getComputedStyle(mk('color: contrast-color(white)')).color);
  k('cc-on-black', getComputedStyle(mk('background: black; color: contrast-color(black)')).color);
  k('cc-overrides', getComputedStyle(mk('color: red; color: contrast-color(white)')).color);
})();

document.getElementById('out').textContent=r.join('\n');
</script></body></html>
"####;

/// One test in the binary — see the module note.
#[test]
fn the_capability_map_unknowns_have_a_measured_verdict() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://constellation-probe.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CONSTELLATION PROBE:\n{got}");

    // Every row headless-Chrome-arbitrated on the same page; a `// Chrome:` note marks a divergence,
    // and the ONLY one of those we are ahead on is contrast-color (see the module note).
    for (name, expect) in [
        ("font-feature-settings", "UNDEF"),     // Chrome: "kern", "liga" 0
        ("text-decoration-style", "UNDEF"),     // Chrome: wavy
        ("text-decoration-thickness", "UNDEF"), // Chrome: 3px
        ("text-decoration-skip-ink", "UNDEF"),  // Chrome: auto
        ("animation-delay", "UNDEF"),           // Chrome: 2.5s
        ("offset-path", "UNDEF"),               // Chrome: path("M 0 0 L 100 100")
        ("offset-distance", "UNDEF"),           // Chrome: 50%
        ("offset-rotate", "UNDEF"),             // Chrome: 45deg
        ("relative-color", "color(srgb 1 0 0 / 0.5)"),
        ("ic-unit", "160px"),
        ("break-before", "UNDEF"),                       // Chrome: page
        ("break-inside", "UNDEF"),                       // Chrome: avoid
        ("column-rule-color", "UNDEF"),                  // Chrome: rgb(255, 0, 0)
        ("column-rule-style", "UNDEF"),                  // Chrome: solid
        ("column-rule-width", "UNDEF"),                  // Chrome: 2px
        ("radial-closest-corner", "radial-gradient(…)"), // Chrome: radial-gradient(closest-corner at 30% 40%, rgb(255, 0, 0), rgb(0, 0, 255))
        ("caret-color", "UNDEF"),                        // Chrome: rgb(0, 128, 0)
        ("image-orientation", "UNDEF"),                  // Chrome: none
        ("text-emphasis-style", "UNDEF"),                // Chrome: dot
        ("paint-order", "UNDEF"),                        // Chrome: stroke
        ("box-decoration-break", "UNDEF"),               // Chrome: clone
        ("anchor-name", "UNDEF"),                        // Chrome: --a
        ("position-area", "UNDEF"),                      // Chrome: left top
        ("sup:if()", "no"),                              // Chrome: yes
        ("sup:contrast-color", "yes"),                   // Chrome: no
        ("sup:shape()", "no"),                           // Chrome: yes
        ("sup:zoom", "yes"),
        ("sup:container-style", "yes"),
        ("sup:scroll-marker", "no"),   // Chrome: yes
        ("sup:gap-decorations", "no"), // Chrome: yes
        ("sup:update-media", "yes"),
        ("sup:device-width", "yes"),
        ("PressureObserver", "absent"), // Chrome: present
        ("AudioContext", "absent"),     // Chrome: present
        ("Translator", "absent"),       // Chrome: present
        ("LanguageDetector", "absent"), // Chrome: present
        ("Summarizer", "absent"),       // Chrome: present
        ("serviceWorker", "present"),
        ("ClipboardEvent", "present"),
        ("execCommand", "present"),
        ("queryCommandSupported-undo", "false"), // Chrome: true
        ("input.minLength", "2/8"),
        ("dialog.closedBy", "no"),          // Chrome: yes
        ("button.command", "no"),           // Chrome: yes
        ("button.commandForElement", "no"), // Chrome: yes
        ("interestTargetElement", "no"),
        ("writingSuggestions", "no"),    // Chrome: yes
        ("img.sizes-auto", "undefined"), // Chrome: auto
        ("CustomElementRegistry", "present"),
        ("WebTransport", "absent"),           // Chrome: present
        ("RTCPeerConnection", "absent"),      // Chrome: present
        ("WebAssembly.Suspending", "absent"), // Chrome: present
        ("Highlight", "absent"),              // Chrome: present
        ("navigation", "present"),
        ("select-multiple-taller", "yes"),
        ("cc-on-white", "rgb(0, 0, 0)"),
        ("cc-on-black", "rgb(255, 255, 255)"), // Chrome: rgb(0, 0, 0)
        ("cc-overrides", "rgb(0, 0, 0)"),      // Chrome: rgb(255, 0, 0)
    ] {
        let needle = format!("{name}={expect}");
        assert!(
            got.contains(&needle),
            "G_CONSTELLATION_UNKNOWNS: expected `{needle}`\n  got: {got}\n\n  \
             This gate PINS the measured verdict for a capability the map used to carry as `unknown`. \
             A red row here is not necessarily a regression — it may be a capability that just \
             ARRIVED. Re-measure the row against headless Chrome, update this claim, and update the \
             matching row in docs/loop/CONSTELLATION.tsv in the SAME tick. What must never happen is \
             the claim being loosened so the map can stay stale."
        );
    }
}
