//! **G_WEB_WORKER — the worker script runs, and the answer comes back.**
//!
//! **The failure this gate exists for.** `new Worker(url)` constructed an object and then fired
//! `error` on the next turn — the shape of a worker script that 404s. That was *honest*, and it was
//! also a dead end for every page whose real work happens off the main thread: the markdown never
//! renders, the search index never builds, the diff never returns, the spreadsheet never
//! recalculates. The page does not crash; it sits on its spinner, because a library's `onerror`
//! path is almost always "surface the failure", not "redo it inline".
//!
//! **What is asserted, and what each claim would catch alone:**
//!
//!   * `ready` / `sum` — the script *evaluated* and *computed*. A shim that constructs a plausible
//!     Worker object and never runs the source passes every API-shaped assertion and fails here.
//!     `500500` is a number nothing but the actual loop produces.
//!   * `nodoc` / `nowin` / `nols` — the worker cannot see `document`, `window` or `localStorage`.
//!     This is the claim that matters most and looks least important: `typeof document ===
//!     'undefined'` is how essentially every isomorphic module decides which half of itself to run,
//!     so a worker scope that leaks the page's globals makes that decision *wrong* and then lets
//!     the wrong branch touch a DOM that must not be there. An implementation that evaluates the
//!     script in the page's own scope passes `sum` and fails these three.
//!   * `echo` / `mutated` — messages carry a **structured clone taken at post time**. The page
//!     mutates the payload on the line after `postMessage`; if the worker sees the mutation, the
//!     reference was passed, and two sides now share state that the spec says they do not.
//!   * `noleak` — a `var` in the worker does not become a page global.
//!   * `errored` — a worker whose script throws surfaces `error` on the worker object. Swallowing
//!     it is the other way to make a page wait forever.
//!   * `afterterminate` — nothing is delivered after `terminate()`. A terminate that lets one more
//!     message through resurrects the exact work the page just cancelled.
//!   * `sharedhonest` — `SharedWorker` is NOT aliased to the real `Worker`. It is not implemented,
//!     and a row that says "half of this works" by aliasing is how a capability board goes stale.
//!
//! RED, on load-bearing claims rather than on the gate as a whole: restoring the error-stub drops
//! `ready`/`sum`/`echo` together; dropping the DOM deny-list flips `nodoc`/`nowin`/`nols` while
//! `sum` still passes — which is precisely the half-working state that is hardest to see from the
//! API surface; cloning at delivery instead of at post time flips `mutated`; `terminate()` as a
//! no-op flips `afterterminate`.
//!
//! **A claim that was WRITTEN AND THEN REMOVED, because it could not fail.** The scope object is
//! built with `Object.create(null)` so that no `Object.prototype` name (`constructor`, `toString`,
//! `valueOf`) can shadow a real global inside someone's library. Two probes were written to assert
//! that — `constructor === Object` and `__proto__ === Object.prototype` — and *both came back
//! identical under a plain-object scope and a null-prototype one*, because the page's own global
//! inherits from `Object.prototype` too, so the `with` fall-through resolves those names to the
//! very same members. The null prototype stays: it is the right defensive choice and costs nothing.
//! The assertion does not, because an assertion that cannot go red is not evidence — it is
//! decoration that later reads as coverage.
//!
//! HONEST LIMIT, gated nowhere because it is a divergence and not a bug: there is no second thread.
//! A worker that spins does not keep the UI responsive. What this buys is that the work COMPLETES
//! and the answer ARRIVES.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } },
    };

    try {
      // The worker script. `probe` is captured at evaluation time, before any message arrives, so
      // it reports the scope the script was BORN in rather than one repaired on first use.
      var SRC = [
        "var secret = 42;",
        "var probe = {",
        "  doc: typeof document,",
        "  win: typeof window,",
        "  ls: typeof localStorage,",
        "  selfIsGlobal: (self === globalThis),",
        "  hasFetch: typeof fetch",
        "};",
        "self.onmessage = function (e) {",
        "  var d = e.data;",
        "  if (d && d.op === 'sum') {",
        "    var s = 0; for (var i = 1; i <= d.n; i++) { s += i; }",
        "    postMessage({ op: 'sum', value: s, echo: d.payload, probe: probe });",
        "  } else if (d && d.op === 'ping') {",
        "    postMessage({ op: 'pong' });",
        "  }",
        "};",
        "postMessage({ op: 'ready' });"
      ].join('\n');

      var url = URL.createObjectURL(new Blob([SRC]));
      var w = new Worker(url);

      var sawReady = false, sawPong = false;

      w.onmessage = function (e) {
        var d = e.data;
        if (d.op === 'ready') { sawReady = true; return; }
        if (d.op === 'pong') { sawPong = true; return; }
        if (d.op === 'sum') {
          R.push('ready:' + sawReady);
          // Nothing but the real loop produces this number.
          R.push('sum:' + (d.value === 500500));

          // The worker scope is not the page scope.
          R.push('nodoc:' + (d.probe.doc === 'undefined'));
          R.push('nowin:' + (d.probe.win === 'undefined'));
          R.push('nols:' + (d.probe.ls === 'undefined'));
          // ...but it is not a bare sandbox either: the platform globals a worker DOES have are
          // still there. Without this, "deny everything" would pass the three claims above.
          R.push('hasfetch:' + (d.probe.hasFetch === 'function'));
          R.push('selfglobal:' + (d.probe.selfIsGlobal === true));
          // The payload survived the crossing intact...
          R.push('echo:' + (d.echo && d.echo.nested && d.echo.nested.k === 'v' &&
                            d.echo.list && d.echo.list.length === 3 && d.echo.list[2] === 3));
          // ...and did NOT see the mutation the page made after posting it.
          R.push('mutated:' + (d.echo.nested.k !== 'MUTATED' && d.echo.list.length === 3));

          // A `var` inside the worker is not a page global.
          R.push('noleak:' + (typeof secret === 'undefined'));

          // SharedWorker must not be quietly aliased to the working Worker.
          R.push('sharedhonest:' + (globalThis.SharedWorker !== Worker));

          // ── terminate() is final. Post after it, then let several turns elapse.
          w.terminate();
          w.postMessage({ op: 'ping' });
          setTimeout(function () { setTimeout(function () { setTimeout(function () {
            R.push('afterterminate:' + (sawPong === false));
          }, 0); }, 0); }, 0);
        }
      };

      var payload = { list: [1, 2, 3], nested: { k: 'v' } };
      w.postMessage({ op: 'sum', n: 1000, payload: payload });
      // Mutation AFTER the post — a reference-passing implementation shows this to the worker.
      payload.nested.k = 'MUTATED';
      payload.list.push(99);

      // ── A worker whose script throws surfaces the failure on the worker object.
      var bad = new Worker(URL.createObjectURL(new Blob(["throw new Error('boom');"])));
      bad.onerror = function (ev) {
        R.push('errored:' + (String(ev.message).indexOf('boom') >= 0));
      };
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

/// One test in the binary — a second `Page::load` stands up a second SpiderMonkey context in the
/// same process and does not survive teardown (see `g_mse`, `g_canvas_text`, `g_globals`).
#[test]
fn a_web_worker_runs_its_script_in_its_own_scope_and_messages_round_trip() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://worker.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("ready:true", "the worker script must EVALUATE — its unprompted `ready` post is the only proof the source ran at all"),
        ("sum:true", "the worker must actually COMPUTE and return the result; 500500 is a number nothing but the real loop produces"),
        ("nodoc:true", "`typeof document` must be 'undefined' inside a worker — it is how nearly every isomorphic module picks which half of itself to run"),
        ("nowin:true", "`window` must not be reachable from a worker scope"),
        ("nols:true", "`localStorage` is main-thread-only; a worker that sees it takes the wrong branch and then writes to it"),
        ("hasfetch:true", "the deny-list must be a deny-list: `fetch` and the rest of the platform still resolve inside the worker"),
        ("selfglobal:true", "`self === globalThis` inside a worker — the scope is the worker's global, not a side object"),
        ("echo:true", "the posted payload must arrive structurally intact, nested objects and arrays included"),
        ("mutated:true", "the clone is taken at POST time — mutating the payload afterwards must not change what the worker receives"),
        ("noleak:true", "a `var` declared in the worker script must not become a page global"),
        ("sharedhonest:true", "`SharedWorker` is not implemented and must not be aliased to the working `Worker`"),
        ("afterterminate:true", "nothing may be delivered after `terminate()` — one more message resurrects the work the page just cancelled"),
        ("errored:true", "a worker whose script throws must surface `error` on the worker object; swallowing it makes the page wait forever"),
    ] {
        assert!(
            got.contains(claim),
            "G_WEB_WORKER: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
