//! **`document.URL` / `documentURI` and `node.baseURI` — on the PROTOTYPES, so a document that is
//! not `window.document` has them too.**
//!
//! ⚠⚠⚠ **`baseURI` DID NOT EXIST ANYWHERE, INCLUDING ON THE MAIN DOCUMENT.** Probed on a page loaded
//! from `https://dp.test/dir/page.html`:
//!
//! ```text
//!   document.URL                       "https://dp.test/dir/page.html"   ✓
//!   document.baseURI                    undefined                        ✗   <- the MAIN document
//!   new DOMParser().parseFromString(…).URL          undefined            ✗
//!   …                              .documentURI     undefined            ✗
//!   …                              .baseURI         undefined            ✗
//! ```
//!
//! `URL` and `documentURI` were defined, but as **own properties of `g.document`** — so every
//! document that is not the window's had none of them. `baseURI` is a **`Node`** property (every
//! node has one) and was simply never built, which is why `reflect_js.rs` already carries the
//! work-around `new URL(raw, document.baseURI || location.href)`: the `|| location.href` half is
//! that absence, written down and routed around rather than fixed.
//!
//! **Where they belong.** `URL`/`documentURI` go on `Document.prototype` and `baseURI` on
//! `Node.prototype`, which is the same placement `defaultView` (iframe_js) and the collections
//! already use. The existing own-properties on `g.document` still shadow these for the main
//! document — deliberately, because those are accessors onto the live `g.location` and must keep
//! winning across an SPA `pushState`.
//!
//! **What a DOMParser document's URL IS.** Per DOM §DOMParser, the created document's URL is the
//! URL of the **responsible document** — the document that created the parser — not `about:blank`
//! and not empty. In this engine that is `g.location.href`, which is what the getter returns.
//!
//! **What `baseURI` IS.** The node document's *base URL*: its own `<base href>` resolved against the
//! document URL, or the document URL when there is none (HTML §2.4.1). Resolved against
//! `location.href` rather than assumed absolute, because `<base href="/x/">` is the common spelling.
//!
//! ⚠ **Scope, stated rather than implied:** this reads the base element from the node's OWN
//! document, so a `<base>` inside a DOMParser-created document is honoured for nodes in that
//! document. It does not model the full "fallback base URL" chain for `about:blank` / `srcdoc`
//! frames, which inherit through the browsing context — those already receive the embedder's URL
//! from `render_iframe`, so the common case is right and the exotic one is named here rather than
//! silently approximated.

pub const DOCUMENT_URL_JS: &str = r#"
(function () {
  var g = globalThis;
  var DP = (typeof Document !== 'undefined' && Document.prototype) || null;
  var NP = (typeof Node !== 'undefined' && Node.prototype) || null;

  // The responsible document's URL. `g.location` is replaced wholesale on every SPA navigation
  // (`__applyUrl`), so this must READ it each time rather than close over an href.
  function docUrl() {
    try { return (g.location && g.location.href) || ''; } catch (e) { return ''; }
  }

  if (DP && !('URL' in DP)) {
    Object.defineProperty(DP, 'URL', {
      configurable: true,
      get: function () { return docUrl(); }
    });
  }
  if (DP && !('documentURI' in DP)) {
    Object.defineProperty(DP, 'documentURI', {
      configurable: true,
      get: function () { return docUrl(); }
    });
  }

  if (NP && !('baseURI' in NP)) {
    Object.defineProperty(NP, 'baseURI', {
      configurable: true,
      get: function () {
        var url = docUrl();
        // The node's OWN document — a node in a DOMParser-created document must read that
        // document's <base>, not the window's.
        var doc = null;
        try { doc = this.ownerDocument || (this.nodeType === 9 ? this : null); } catch (e) {}
        if (!doc || !doc.querySelector) { return url; }
        var b = null;
        try { b = doc.querySelector('base[href]'); } catch (e) {}
        if (!b) { return url; }
        var href = '';
        try { href = b.getAttribute('href') || ''; } catch (e) {}
        if (!href) { return url; }
        // `<base href="/x/">` is the common spelling, so resolve rather than assume absolute.
        try { return new URL(href, url).href; } catch (e) { return href; }
      }
    });
  }
})();
"#;
