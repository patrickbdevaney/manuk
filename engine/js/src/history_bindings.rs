//! N2 — the **History API** (`pushState` / `replaceState` / `go` / `popstate` /
//! `hashchange`), bound over N1's one [`SessionHistory`].
//!
//! The spec rules this enforces, exactly (WHATWG HTML §7.2, nav-history-apis):
//!
//! * `pushState(data, unused, url)` appends a session-history entry;
//!   `replaceState(data, unused, url)` mutates the current one.
//! * **Neither fetches or loads anything.** The document URL changes; the document does
//!   not. This is the entire reason client-side routing works, and it is why there is no
//!   network call anywhere below.
//! * **Neither fires `popstate`.** `popstate` fires only on *traversal* (`back`/`forward`/
//!   `go`). Frameworks depend on this asymmetry — firing it on `pushState` would make a
//!   router recurse.
//! * The target URL must be **same-origin** with the current document, else `SecurityError`.
//! * A traversal whose URL differs from the current one *only in fragment* additionally
//!   fires `hashchange`.
//!
//! **Why this needs no engine work:** the History API is pure host state. Nothing here
//! touches SpiderMonkey's JIT, GC, or sandbox — it is `JS_DefineFunction` /
//! `JS_DefineProperty1` over a reserved-slot host pointer, the sanctioned embedding
//! surface, exactly as `dom_bindings` does. The model ([`SessionHistory`]) is not
//! feature-gated at all; only these bindings are.
//!
//! **Documented gaps (not faked):** `history.scrollRestoration` is absent; state objects
//! are round-tripped through JSON, so non-JSON-serializable values (functions, cycles,
//! `Map`) are lost rather than structured-cloned — the spec calls for StructuredClone;
//! the Navigation API (`navigation.navigate`, `navigate` events) is out of scope.

use std::ptr::{self, NonNull};

use mozjs::context::JSContext;
use mozjs::glue::JS_GetReservedSlot;
use mozjs::jsapi::{
    JSClass, JSContext as RawJSContext, JSObject, JS_SetReservedSlot, Value, JSPROP_ENUMERATE,
};
use mozjs::jsval::{Int32Value, ObjectValue, PrivateValue, UndefinedValue};
use mozjs::rooted;
use mozjs::rust::wrappers2::{
    CurrentGlobalOrNull, JS_DefineFunction, JS_DefineProperty1, JS_NewObject, JS_SetProperty,
};

pub use crate::history_host::{HistoryHost, TraversalEvents};

const SLOT_HOST: u32 = 0; // *mut HistoryHost
const NUM_SLOTS: u32 = 1;
const RESERVED_SLOTS_SHIFT: u32 = 8;

static HISTORY_CLASS: JSClass = JSClass {
    name: c"History".as_ptr(),
    flags: NUM_SLOTS << RESERVED_SLOTS_SHIFT,
    cOps: ptr::null(),
    spec: ptr::null(),
    ext: ptr::null(),
    oOps: ptr::null(),
};

static LOCATION_CLASS: JSClass = JSClass {
    name: c"Location".as_ptr(),
    flags: NUM_SLOTS << RESERVED_SLOTS_SHIFT,
    cOps: ptr::null(),
    spec: ptr::null(),
    ext: ptr::null(),
    oOps: ptr::null(),
};

// ---------------------------------------------------------------------------
// JS bindings
// ---------------------------------------------------------------------------

unsafe fn wrap_cx(cx: *mut RawJSContext) -> JSContext {
    JSContext::from_ptr(NonNull::new(cx).expect("native cx is non-null"))
}

/// Read the host pointer out of `this`'s reserved slot. Returns `None` (never a segfault)
/// when `this` is not one of our reflectors.
unsafe fn host_of<'a>(obj: *mut JSObject) -> Option<&'a mut HistoryHost> {
    if obj.is_null() {
        return None;
    }
    let mut slot = UndefinedValue();
    JS_GetReservedSlot(obj, SLOT_HOST, &mut slot);
    if slot.is_undefined() {
        return None;
    }
    let p = slot.to_private() as *mut HistoryHost;
    if p.is_null() {
        return None;
    }
    Some(&mut *p)
}

/// `this` for a native call lives in `vp[1]` (the same convention `dom_bindings` uses).
unsafe fn this_obj(vp: *mut Value) -> *mut JSObject {
    let this = *vp.offset(1);
    if this.is_object() {
        this.to_object()
    } else {
        ptr::null_mut()
    }
}

unsafe fn arg(vp: *mut Value, argc: u32, i: u32) -> Value {
    if i < argc {
        *vp.offset(2 + i as isize)
    } else {
        UndefinedValue()
    }
}

unsafe fn set_rval(vp: *mut Value, v: Value) {
    *vp = v;
}

fn escape_js(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

unsafe fn eval(cx: *mut RawJSContext, script: &str) -> Option<Value> {
    use mozjs::rust::{evaluate_script, CompileOptionsWrapper};
    let global = CurrentGlobalOrNull(&wrap_cx(cx));
    if global.is_null() {
        return None;
    }
    rooted!(in(cx) let g = global);
    rooted!(in(cx) let mut rval = UndefinedValue());
    let opts = CompileOptionsWrapper::new(&wrap_cx(cx), c"history.js".to_owned(), 1);
    evaluate_script(
        &mut wrap_cx(cx),
        g.handle(),
        script,
        rval.handle_mut(),
        opts,
    )
    .ok()?;
    Some(rval.get())
}

/// Serialize a JS value to a JSON string by round-tripping through `JSON.stringify` in
/// the global. Documented gap: this is JSON, not StructuredClone.
unsafe fn stringify_arg(cx: *mut RawJSContext, vp: *mut Value, argc: u32, i: u32) -> String {
    let v = arg(vp, argc, i);
    if v.is_undefined() || v.is_null() {
        return "null".to_string();
    }
    // Stash the value on the global, stringify it there, and read it back as a string.
    rooted!(in(cx) let mut slot = v);
    let global = CurrentGlobalOrNull(&wrap_cx(cx));
    if global.is_null() {
        return "null".to_string();
    }
    rooted!(in(cx) let g = global);
    if !JS_SetProperty(
        &mut wrap_cx(cx),
        g.handle(),
        c"__histTmp".as_ptr(),
        slot.handle(),
    ) {
        return "null".to_string();
    }
    match eval(cx, "JSON.stringify(globalThis.__histTmp) ?? 'null'") {
        Some(rv) if rv.is_string() => js_string_to_rust(cx, rv),
        _ => "null".to_string(),
    }
}

unsafe fn js_string_to_rust(cx: *mut RawJSContext, v: Value) -> String {
    use mozjs::conversions::{ConversionResult, FromJSValConvertible};
    rooted!(in(cx) let rv = v);
    let mut c = wrap_cx(cx);
    match String::safe_from_jsval(&mut c, rv.handle(), ()) {
        Ok(ConversionResult::Success(s)) => s,
        _ => String::new(),
    }
}

unsafe fn opt_string_arg(
    cx: *mut RawJSContext,
    vp: *mut Value,
    argc: u32,
    i: u32,
) -> Option<String> {
    let v = arg(vp, argc, i);
    if v.is_undefined() || v.is_null() {
        return None;
    }
    if v.is_string() {
        return Some(js_string_to_rust(cx, v));
    }
    None
}

unsafe fn throw(cx: *mut RawJSContext, msg: &str) -> bool {
    let script = format!("(function(){{ throw new Error({}); }})()", escape_js(msg));
    eval(cx, &script);
    false
}

// ---- natives ----

unsafe extern "C" fn history_push_state(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some(host) = host_of(this_obj(vp)) else {
        set_rval(vp, UndefinedValue());
        return true;
    };
    let state = stringify_arg(cx, vp, argc, 0);
    let url = opt_string_arg(cx, vp, argc, 2);
    // NOTE: no fetch, no popstate. Both are load-bearing spec requirements.
    match host.push_state(&state, url.as_deref()) {
        Ok(()) => {
            set_rval(vp, UndefinedValue());
            true
        }
        Err(e) => throw(cx, &e),
    }
}

unsafe extern "C" fn history_replace_state(
    cx: *mut RawJSContext,
    argc: u32,
    vp: *mut Value,
) -> bool {
    let Some(host) = host_of(this_obj(vp)) else {
        set_rval(vp, UndefinedValue());
        return true;
    };
    let state = stringify_arg(cx, vp, argc, 0);
    let url = opt_string_arg(cx, vp, argc, 2);
    match host.replace_state(&state, url.as_deref()) {
        Ok(()) => {
            set_rval(vp, UndefinedValue());
            true
        }
        Err(e) => throw(cx, &e),
    }
}

/// Shared by `go`/`back`/`forward`: traverse, then fire the events the spec requires.
unsafe fn do_traverse(cx: *mut RawJSContext, vp: *mut Value, delta: i64) -> bool {
    let Some(host) = host_of(this_obj(vp)) else {
        set_rval(vp, UndefinedValue());
        return true;
    };
    // An out-of-range traversal moves nowhere and fires nothing.
    let Some(events) = host.traverse(delta) else {
        set_rval(vp, UndefinedValue());
        return true;
    };
    let state = host.state_json().to_string();
    let url = host.current_url().to_string();

    // `popstate` always; `hashchange` too when only the fragment changed.
    let mut script = format!(
        "globalThis.__fireWindowEvent('popstate', {{type:'popstate', state: JSON.parse({})}});",
        escape_js(&state)
    );
    if events == TraversalEvents::PopStateAndHashChange {
        script.push_str(&format!(
            "globalThis.__fireWindowEvent('hashchange', {{type:'hashchange', newURL: {}}});",
            escape_js(&url)
        ));
    }
    eval(cx, &script);
    set_rval(vp, UndefinedValue());
    true
}

unsafe extern "C" fn history_go(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let v = arg(vp, argc, 0);
    let delta = if v.is_int32() {
        v.to_int32() as i64
    } else if v.is_double() {
        v.to_double() as i64
    } else {
        0
    };
    do_traverse(cx, vp, delta)
}

unsafe extern "C" fn history_back(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    do_traverse(cx, vp, -1)
}

unsafe extern "C" fn history_forward(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    do_traverse(cx, vp, 1)
}

unsafe extern "C" fn history_length_get(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    let n = host_of(this_obj(vp)).map(|h| h.len()).unwrap_or(0);
    let _ = cx;
    set_rval(vp, Int32Value(n as i32));
    true
}

unsafe extern "C" fn history_state_get(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    let json = host_of(this_obj(vp))
        .map(|h| h.state_json().to_string())
        .unwrap_or_else(|| "null".to_string());
    match eval(cx, &format!("JSON.parse({})", escape_js(&json))) {
        Some(v) => set_rval(vp, v),
        None => set_rval(vp, UndefinedValue()),
    }
    true
}

unsafe extern "C" fn location_href_get(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    let s = host_of(this_obj(vp))
        .map(|h| h.current_url().to_string())
        .unwrap_or_default();
    to_js_string(cx, vp, &s)
}

unsafe extern "C" fn location_pathname_get(
    cx: *mut RawJSContext,
    _argc: u32,
    vp: *mut Value,
) -> bool {
    let s = host_of(this_obj(vp))
        .map(|h| h.current_url().path().to_string())
        .unwrap_or_default();
    to_js_string(cx, vp, &s)
}

unsafe extern "C" fn location_search_get(
    cx: *mut RawJSContext,
    _argc: u32,
    vp: *mut Value,
) -> bool {
    let s = host_of(this_obj(vp))
        .map(|h| {
            h.current_url()
                .query()
                .map(|q| format!("?{q}"))
                .unwrap_or_default()
        })
        .unwrap_or_default();
    to_js_string(cx, vp, &s)
}

unsafe extern "C" fn location_hash_get(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    let s = host_of(this_obj(vp))
        .map(|h| {
            h.current_url()
                .fragment()
                .map(|f| format!("#{f}"))
                .unwrap_or_default()
        })
        .unwrap_or_default();
    to_js_string(cx, vp, &s)
}

unsafe fn to_js_string(cx: *mut RawJSContext, vp: *mut Value, s: &str) -> bool {
    use mozjs::conversions::ToJSValConvertible;
    rooted!(in(cx) let mut out = UndefinedValue());
    s.to_jsval(cx, out.handle_mut());
    set_rval(vp, out.get());
    true
}

/// The JS-side window-event shim: `addEventListener` + `on<type>` handlers, driven by
/// `__fireWindowEvent`. Kept in JS so the natives never have to root a callback.
const WINDOW_EVENTS_SHIM: &str = r#"
globalThis.__winListeners = globalThis.__winListeners || {};
globalThis.addEventListener = function (type, fn) {
  (globalThis.__winListeners[type] = globalThis.__winListeners[type] || []).push(fn);
};
globalThis.removeEventListener = function (type, fn) {
  var l = globalThis.__winListeners[type] || [];
  var i = l.indexOf(fn);
  if (i >= 0) l.splice(i, 1);
};
globalThis.__fireWindowEvent = function (type, ev) {
  var n = 0;
  var h = globalThis['on' + type];
  if (typeof h === 'function') { h(ev); n++; }
  var l = globalThis.__winListeners[type] || [];
  for (var i = 0; i < l.length; i++) { l[i](ev); n++; }
  return n;
};
"#;

/// Install `history` and `location` on the global, backed by `host`.
///
/// # Safety
/// `host` must outlive the `Runtime` (the reflectors hold a raw pointer to it), and the
/// embedder must not move it. Single-threaded use, as with `dom_bindings`.
pub unsafe fn install(
    cx: *mut RawJSContext,
    global: mozjs::rust::HandleObject,
    host: *mut HistoryHost,
) -> Result<(), String> {
    // The window-event shim must exist before any traversal can fire into it.
    if eval(cx, WINDOW_EVENTS_SHIM).is_none() {
        return Err("installing the window-event shim failed".to_string());
    }

    let def_fn = |obj: &mozjs::rust::RootedGuard<'_, *mut JSObject>,
                  name: &std::ffi::CStr,
                  f: unsafe extern "C" fn(*mut RawJSContext, u32, *mut Value) -> bool,
                  n: u32| {
        JS_DefineFunction(&mut wrap_cx(cx), obj.handle(), name.as_ptr(), Some(f), n, 0);
    };
    let def_get =
        |obj: &mozjs::rust::RootedGuard<'_, *mut JSObject>,
         name: &std::ffi::CStr,
         g: unsafe extern "C" fn(*mut RawJSContext, u32, *mut Value) -> bool| {
            JS_DefineProperty1(
                &mut wrap_cx(cx),
                obj.handle(),
                name.as_ptr(),
                Some(g),
                None,
                JSPROP_ENUMERATE as u32,
            );
        };

    // ---- history ----
    rooted!(in(cx) let hist = JS_NewObject(&mut wrap_cx(cx), &HISTORY_CLASS));
    if hist.get().is_null() {
        return Err("JS_NewObject(History) failed".to_string());
    }
    JS_SetReservedSlot(hist.get(), SLOT_HOST, &PrivateValue(host as *const _));
    def_fn(&hist, c"pushState", history_push_state, 3);
    def_fn(&hist, c"replaceState", history_replace_state, 3);
    def_fn(&hist, c"go", history_go, 1);
    def_fn(&hist, c"back", history_back, 0);
    def_fn(&hist, c"forward", history_forward, 0);
    def_get(&hist, c"length", history_length_get);
    def_get(&hist, c"state", history_state_get);

    rooted!(in(cx) let hist_val = ObjectValue(hist.get()));
    if !JS_SetProperty(
        &mut wrap_cx(cx),
        global,
        c"history".as_ptr(),
        hist_val.handle(),
    ) {
        return Err("defining globalThis.history failed".to_string());
    }

    // ---- location (read-only; enough for a router to observe the URL) ----
    rooted!(in(cx) let loc = JS_NewObject(&mut wrap_cx(cx), &LOCATION_CLASS));
    if loc.get().is_null() {
        return Err("JS_NewObject(Location) failed".to_string());
    }
    JS_SetReservedSlot(loc.get(), SLOT_HOST, &PrivateValue(host as *const _));
    def_get(&loc, c"href", location_href_get);
    def_get(&loc, c"pathname", location_pathname_get);
    def_get(&loc, c"search", location_search_get);
    def_get(&loc, c"hash", location_hash_get);

    rooted!(in(cx) let loc_val = ObjectValue(loc.get()));
    if !JS_SetProperty(
        &mut wrap_cx(cx),
        global,
        c"location".as_ptr(),
        loc_val.handle(),
    ) {
        return Err("defining globalThis.location failed".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod js_tests {
    //! One Runtime, run isolated (SpiderMonkey does not support multiple `Runtime`
    //! create/destroy cycles per process — the same constraint `dom_bindings` documents).
    use super::*;
    use mozjs::jsapi::OnNewGlobalHookOption;
    use mozjs::rust::wrappers2::JS_NewGlobalObject;
    use mozjs::rust::{
        evaluate_script, CompileOptionsWrapper, RealmOptions, Runtime, SIMPLE_GLOBAL_CLASS,
    };

    fn eval_val(
        rt: &mut Runtime,
        global: mozjs::rust::HandleObject,
        src: &str,
    ) -> Result<Value, String> {
        rooted!(&in(rt.cx()) let mut rval = UndefinedValue());
        let opts = CompileOptionsWrapper::new(rt.cx_no_gc(), c"hist.js".to_owned(), 1);
        evaluate_script(rt.cx(), global, src, rval.handle_mut(), opts)
            .map_err(|()| format!("evaluate_script failed for: {src}"))?;
        Ok(rval.get())
    }

    fn is_true(v: Value) -> bool {
        v.is_boolean() && v.to_boolean()
    }

    /// N2's behavior acceptance, through **real JavaScript**: `pushState` changes the URL
    /// with **no network request** and **no `popstate`**; `back()` then fires **exactly
    /// one** `popstate` carrying the right state; a cross-origin URL throws and moves
    /// nothing; `replaceState` adds no entry; a fragment-only traversal also fires
    /// `hashchange`.
    #[test]
    #[ignore = "SpiderMonkey multi-Runtime-per-process teardown; run in isolation"]
    fn history_api_over_the_session_history_model() {
        let mut host = HistoryHost::new("https://ex.test/start").unwrap();
        let host_ptr: *mut HistoryHost = &mut host;

        let handle = crate::spidermonkey::engine_handle().unwrap();
        let mut runtime = Runtime::new(handle);
        let options = RealmOptions::default();
        rooted!(&in(runtime.cx()) let global = unsafe {
            JS_NewGlobalObject(runtime.cx(), &SIMPLE_GLOBAL_CLASS, ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook, &*options)
        });
        let raw_cx = unsafe { runtime.cx().raw_cx() };
        let _ar = mozjs::jsapi::JSAutoRealm::new(raw_cx, global.get());
        unsafe { install(raw_cx, global.handle(), host_ptr) }.unwrap();

        eval_val(
            &mut runtime,
            global.handle(),
            "globalThis.pops = 0; globalThis.hashes = 0; globalThis.lastState = undefined;
             addEventListener('popstate', function (e) { pops++; lastState = e.state; });
             addEventListener('hashchange', function () { hashes++; });",
        )
        .unwrap();

        // pushState: the URL changes; no popstate fires.
        let r = eval_val(
            &mut runtime,
            global.handle(),
            "history.pushState({page: 1}, '', '/one');
             (location.pathname === '/one' && pops === 0 && history.length === 2)",
        )
        .unwrap();
        assert!(
            is_true(r),
            "pushState must change the URL and not fire popstate"
        );

        // ...and it performed no network request. The host counter is the ground truth.
        assert_eq!(
            unsafe { (*host_ptr).fetches() },
            0,
            "pushState must not fetch"
        );

        // back(): exactly one popstate, carrying the previous entry's state (null).
        let r = eval_val(
            &mut runtime,
            global.handle(),
            "history.back();
             (pops === 1 && lastState === null && location.pathname === '/start' && hashes === 0)",
        )
        .unwrap();
        assert!(is_true(r), "back() must fire exactly one popstate");

        // forward(): the pushed state object round-trips.
        let r = eval_val(
            &mut runtime,
            global.handle(),
            "history.forward();
             (pops === 2 && lastState && lastState.page === 1 && history.state.page === 1)",
        )
        .unwrap();
        assert!(is_true(r), "state must round-trip through a traversal");

        // replaceState: no new entry, no popstate.
        let r = eval_val(
            &mut runtime,
            global.handle(),
            "var before = history.length;
             history.replaceState({page: 9}, '', '/one-b');
             (history.length === before && pops === 2 && location.pathname === '/one-b'
              && history.state.page === 9)",
        )
        .unwrap();
        assert!(
            is_true(r),
            "replaceState must not add an entry or fire popstate"
        );

        // A cross-origin state URL throws and changes nothing.
        let r = eval_val(
            &mut runtime,
            global.handle(),
            "var threw = false;
             try { history.pushState({}, '', 'https://evil.test/x'); } catch (e) { threw = true; }
             (threw && location.pathname === '/one-b')",
        )
        .unwrap();
        assert!(
            is_true(r),
            "a cross-origin state URL must throw and move nothing"
        );

        // An out-of-range traversal moves nowhere and fires nothing.
        let r = eval_val(
            &mut runtime,
            global.handle(),
            "var p = pops; history.go(99); (pops === p && location.pathname === '/one-b')",
        )
        .unwrap();
        assert!(is_true(r), "out-of-range go() must be a no-op");

        // A fragment-only traversal fires hashchange as well as popstate.
        let r = eval_val(
            &mut runtime,
            global.handle(),
            "history.pushState({}, '', '#frag');
             var p = pops, h = hashes;
             history.back();
             (pops === p + 1 && hashes === h + 1)",
        )
        .unwrap();
        assert!(
            is_true(r),
            "a fragment-only traversal must fire hashchange too"
        );
    }
}
