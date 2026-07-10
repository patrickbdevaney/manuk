//! D3 **hand-written DOM binding subset** over the arena `manuk-dom`, on a thin safe
//! helper layer — the Step-0 decision (see [`crate::bindings_prototype`] + CLAUDE.md
//! § D3). This is the first tranche of the jQuery-core surface: the DOM methods that
//! read, mutate, and query the tree.
//!
//! # Design (the "thin safe helper layer")
//!
//! Every DOM object is a SpiderMonkey reflector: a `JSObject` of [`NODE_CLASS`] whose
//! two reserved slots hold the backing `(NodeId, *mut Dom)` — the same reserved-slot
//! mechanism Servo uses, over our lean arena DOM (no `Dom<T>`/`Reflector`/GC roots).
//! The helper layer factors out the realm/rooting/ABI/string boilerplate that Step-0
//! flagged as segfault-prone, so each native method is a few safe lines. Inside a
//! native we only have a raw `*mut JSContext`; [`JSContext::from_ptr`] rebuilds the
//! ergonomic `&mut JSContext` wrapper so the same `wrappers2`/`conversions` idiom as
//! the rest of the crate applies. `this` is read straight from `vp[1]` (the
//! receiver) — no `CallArgs` machinery.
//!
//! # Scope (this tranche)
//!
//! Faithful DOM **methods**: `document.getElementById` / `querySelector` /
//! `createElement`, and `element.appendChild` / `setAttribute` / `getAttribute` /
//! `querySelector`. Accessor **properties** (`textContent`, `tagName`, `id`,
//! `className`), `querySelectorAll` (NodeList), events, and `fetch`/XHR are later
//! tranches (documented in CLAUDE.md § D3).
//!
//! # Safety / production notes
//!
//! - The `*mut Dom` in a slot is dereferenced `unsafe`ly; sound because JS runs
//!   single-threaded on the thread that owns the `Dom`, for the lifetime the
//!   reflectors are reachable. The embedder must not free the `Dom` meanwhile.
//! - Production uses ONE process-global `Runtime` (the C1 model); tests run isolated
//!   (SpiderMonkey multi-`Runtime`-per-process teardown, as in the prototype).
//! - Methods are defined per-reflector for simplicity; production would hang them off
//!   per-interface prototype objects.

use std::ptr::{self, NonNull};

use mozjs::context::JSContext;
use mozjs::conversions::{ConversionResult, FromJSValConvertible, ToJSValConvertible};
use mozjs::glue::JS_GetReservedSlot;
use mozjs::jsapi::{JSClass, JSContext as RawJSContext, JSObject, JS_SetReservedSlot, Value};
use mozjs::jsval::{Int32Value, NullValue, ObjectValue, PrivateValue, UndefinedValue};
use mozjs::rooted;
use mozjs::rust::wrappers2::{JS_DefineFunction, JS_DefineProperty, JS_NewObject};

use manuk_dom::{Dom, NodeId};

// Reserved-slot layout on every DOM reflector.
const SLOT_NODE: u32 = 0; // NodeId, as Int32
const SLOT_DOM: u32 = 1; // *mut Dom, as PrivateValue
const NUM_SLOTS: u32 = 2;
const RESERVED_SLOTS_SHIFT: u32 = 8;

/// The shared JSClass for every DOM reflector (document + elements). Two reserved
/// slots carry `(NodeId, *mut Dom)`; the object's methods select document vs element
/// behaviour.
static NODE_CLASS: JSClass = JSClass {
    name: c"Node".as_ptr(),
    flags: NUM_SLOTS << RESERVED_SLOTS_SHIFT,
    cOps: ptr::null(),
    spec: ptr::null(),
    ext: ptr::null(),
    oOps: ptr::null(),
};

// ---------------------------------------------------------------------------
// Helper layer
// ---------------------------------------------------------------------------

/// Rebuild the ergonomic context wrapper from a native's raw `*mut JSContext`.
/// SpiderMonkey passes a valid cx to every native; the wrapper is a thin handle
/// (no ownership/Drop of the underlying context).
unsafe fn wrap_cx(cx: *mut RawJSContext) -> JSContext {
    JSContext::from_ptr(NonNull::new(cx).expect("native cx is non-null"))
}

/// Read `(dom, node)` from a reflector's reserved slots, or `None` if the object is
/// not a DOM reflector / slots are unset (turns a would-be segfault into a graceful
/// `null`/`undefined`).
unsafe fn node_and_dom(obj: *mut JSObject) -> Option<(*mut Dom, NodeId)> {
    let mut ns = UndefinedValue();
    JS_GetReservedSlot(obj, SLOT_NODE, &mut ns);
    let mut ds = UndefinedValue();
    JS_GetReservedSlot(obj, SLOT_DOM, &mut ds);
    if !ns.is_int32() {
        return None;
    }
    let dom = ds.to_private() as *mut Dom;
    if dom.is_null() {
        return None;
    }
    Some((dom, NodeId(ns.to_int32() as usize)))
}

/// `(dom, node)` for the method receiver (`this`, at `vp[1]`).
unsafe fn this_node(vp: *mut Value) -> Option<(*mut Dom, NodeId)> {
    let this = *vp.add(1);
    if !this.is_object() {
        return None;
    }
    node_and_dom(this.to_object())
}

/// Extract string argument `i` (coercing per JS `ToString`), or `None`.
unsafe fn arg_string(cx: *mut RawJSContext, vp: *mut Value, argc: u32, i: u32) -> Option<String> {
    if i >= argc {
        return None;
    }
    let mut c = wrap_cx(cx);
    rooted!(in(cx) let val = *vp.add(2 + i as usize));
    match String::safe_from_jsval(&mut c, val.handle(), ()) {
        Ok(ConversionResult::Success(s)) => Some(s),
        _ => None,
    }
}

/// Object argument `i` (e.g. a child element), or `None`.
unsafe fn arg_object(vp: *mut Value, argc: u32, i: u32) -> Option<*mut JSObject> {
    if i >= argc {
        return None;
    }
    let v = *vp.add(2 + i as usize);
    if v.is_object() {
        Some(v.to_object())
    } else {
        None
    }
}

/// Set the native return value (`vp[0]`) to a JS string.
unsafe fn return_string(cx: *mut RawJSContext, vp: *mut Value, s: &str) {
    rooted!(in(cx) let mut out = UndefinedValue());
    s.to_jsval(cx, out.handle_mut());
    *vp = out.get();
}

/// Set the return value to a fresh reflector for `node`, or `null`.
unsafe fn return_node_or_null(
    cx: *mut RawJSContext,
    vp: *mut Value,
    dom: *mut Dom,
    node: Option<NodeId>,
) {
    match node {
        Some(n) => *vp = ObjectValue(new_reflector(cx, dom, n)),
        None => *vp = NullValue(),
    }
}

/// Create a DOM reflector for `(dom, node)` and install its element methods. The
/// returned pointer is written straight to a GC-rooted `vp[0]` by callers, with no
/// intervening allocation.
unsafe fn new_reflector(cx: *mut RawJSContext, dom: *mut Dom, node: NodeId) -> *mut JSObject {
    let obj_ptr = JS_NewObject(&mut wrap_cx(cx), &NODE_CLASS);
    rooted!(in(cx) let obj = obj_ptr);
    let node_val = Int32Value(node.0 as i32);
    JS_SetReservedSlot(obj.get(), SLOT_NODE, &node_val);
    let dom_val = PrivateValue(dom as *const std::ffi::c_void);
    JS_SetReservedSlot(obj.get(), SLOT_DOM, &dom_val);
    define_members(cx, &obj, false);
    obj.get()
}

/// Define the DOM methods on the rooted object `obj`. `is_document` selects the
/// document surface; otherwise the element surface.
unsafe fn define_members(
    cx: *mut RawJSContext,
    obj: &mozjs::rust::RootedGuard<'_, *mut JSObject>,
    is_document: bool,
) {
    let def = |name: &std::ffi::CStr,
               f: unsafe extern "C" fn(*mut RawJSContext, u32, *mut Value) -> bool,
               n: u32| {
        JS_DefineFunction(&mut wrap_cx(cx), obj.handle(), name.as_ptr(), Some(f), n, 0);
    };
    if is_document {
        def(c"getElementById", doc_get_by_id, 1);
        def(c"querySelector", doc_query, 1);
        def(c"createElement", doc_create_element, 1);
    } else {
        def(c"appendChild", el_append_child, 1);
        def(c"setAttribute", el_set_attribute, 2);
        def(c"getAttribute", el_get_attribute, 1);
        def(c"querySelector", doc_query, 1);
    }
}

// ---------------------------------------------------------------------------
// Document methods
// ---------------------------------------------------------------------------

/// `document.getElementById(id)` → element reflector | null.
unsafe extern "C" fn doc_get_by_id(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((dom, root)) = this_node(vp) else {
        *vp = NullValue();
        return true;
    };
    let Some(id) = arg_string(cx, vp, argc, 0) else {
        *vp = NullValue();
        return true;
    };
    let found = (*dom)
        .descendants(root)
        .find(|&n| (*dom).element(n).and_then(|e| e.id()) == Some(id.as_str()));
    return_node_or_null(cx, vp, dom, found);
    true
}

/// `document.querySelector(sel)` (also installed on elements) → first match | null.
unsafe extern "C" fn doc_query(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((dom, root)) = this_node(vp) else {
        *vp = NullValue();
        return true;
    };
    let Some(sel) = arg_string(cx, vp, argc, 0) else {
        *vp = NullValue();
        return true;
    };
    let found = manuk_css::query_selector(&*dom, root, &sel);
    return_node_or_null(cx, vp, dom, found);
    true
}

/// `document.createElement(tag)` → new detached element reflector.
unsafe extern "C" fn doc_create_element(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((dom, _)) = this_node(vp) else {
        *vp = NullValue();
        return true;
    };
    let tag = arg_string(cx, vp, argc, 0).unwrap_or_else(|| "div".to_string());
    let node = (*dom).create_element(tag);
    *vp = ObjectValue(new_reflector(cx, dom, node));
    true
}

// ---------------------------------------------------------------------------
// Element methods
// ---------------------------------------------------------------------------

/// `element.appendChild(child)` → the appended child (per DOM spec).
unsafe extern "C" fn el_append_child(_cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((dom, parent)) = this_node(vp) else {
        *vp = UndefinedValue();
        return true;
    };
    match arg_object(vp, argc, 0).and_then(|o| node_and_dom(o).map(|(_, c)| (o, c))) {
        Some((child_obj, child)) => {
            (*dom).append_child(parent, child);
            *vp = ObjectValue(child_obj);
        }
        None => *vp = UndefinedValue(),
    }
    true
}

/// `element.setAttribute(name, value)`.
unsafe extern "C" fn el_set_attribute(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((dom, node)) = this_node(vp) else {
        *vp = UndefinedValue();
        return true;
    };
    if let (Some(name), Some(value)) = (arg_string(cx, vp, argc, 0), arg_string(cx, vp, argc, 1)) {
        (*dom).set_attr(node, name, value);
    }
    *vp = UndefinedValue();
    true
}

/// `element.getAttribute(name)` → string | null.
unsafe extern "C" fn el_get_attribute(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((dom, node)) = this_node(vp) else {
        *vp = NullValue();
        return true;
    };
    let Some(name) = arg_string(cx, vp, argc, 0) else {
        *vp = NullValue();
        return true;
    };
    match (*dom)
        .element(node)
        .and_then(|e| e.attr(&name))
        .map(str::to_owned)
    {
        Some(v) => return_string(cx, vp, &v),
        None => *vp = NullValue(),
    }
    true
}

// ---------------------------------------------------------------------------
// Installation
// ---------------------------------------------------------------------------

/// Install `document` (a reflector for `dom`'s root) onto `global`. The `dom` pointer
/// must stay valid for as long as the runtime can reach the reflectors.
///
/// # Safety
/// `cx`/`global` must be in the same realm; `dom` must be a live `*mut Dom`.
pub unsafe fn install(
    cx: *mut RawJSContext,
    global: &mozjs::rust::RootedGuard<'_, *mut JSObject>,
    dom: *mut Dom,
) {
    let root = (*dom).root();
    let doc_ptr = JS_NewObject(&mut wrap_cx(cx), &NODE_CLASS);
    rooted!(in(cx) let document = doc_ptr);
    let node_val = Int32Value(root.0 as i32);
    JS_SetReservedSlot(document.get(), SLOT_NODE, &node_val);
    let dom_val = PrivateValue(dom as *const std::ffi::c_void);
    JS_SetReservedSlot(document.get(), SLOT_DOM, &dom_val);
    define_members(cx, &document, true);

    rooted!(in(cx) let doc_val = ObjectValue(document.get()));
    JS_DefineProperty(
        &mut wrap_cx(cx),
        global.handle(),
        c"document".as_ptr(),
        doc_val.handle(),
        0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use mozjs::jsapi::OnNewGlobalHookOption;
    use mozjs::rust::wrappers2::JS_NewGlobalObject;
    use mozjs::rust::{
        evaluate_script, CompileOptionsWrapper, RealmOptions, Runtime, SIMPLE_GLOBAL_CLASS,
    };

    /// Evaluate `script` against a global with `document` installed over `dom`,
    /// returning the boolean result.
    fn eval_bool(dom: &mut Dom, script: &str) -> Result<bool, String> {
        let handle = crate::spidermonkey::engine_handle().map_err(|e| e.message)?;
        let mut runtime = Runtime::new(handle);
        let options = RealmOptions::default();
        rooted!(&in(runtime.cx()) let global = unsafe {
            JS_NewGlobalObject(runtime.cx(), &SIMPLE_GLOBAL_CLASS, ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook, &*options)
        });
        let raw_cx = unsafe { runtime.cx().raw_cx() };
        let _ar = mozjs::jsapi::JSAutoRealm::new(raw_cx, global.get());
        unsafe { install(raw_cx, &global, dom as *mut Dom) };

        rooted!(&in(runtime.cx()) let mut rval = UndefinedValue());
        let opts = CompileOptionsWrapper::new(runtime.cx_no_gc(), c"dom.js".to_owned(), 1);
        evaluate_script(
            runtime.cx(),
            global.handle(),
            script,
            rval.handle_mut(),
            opts,
        )
        .map_err(|()| "evaluate_script failed".to_string())?;
        if !rval.get().is_boolean() {
            return Err("result not boolean".to_string());
        }
        Ok(rval.get().to_boolean())
    }

    // ONE test / ONE Runtime: SpiderMonkey does not support multiple Runtime
    // create/destroy cycles per process, so both the prototype and this test use a
    // single Runtime and run isolated:
    //   cargo test -p manuk-js --features spidermonkey dom_bindings -- --ignored
    #[test]
    #[ignore = "SpiderMonkey multi-Runtime-per-process teardown; run in isolation"]
    fn dom_methods_over_arena() {
        // <html><body><p id=greeting></p></body></html>
        let mut dom = Dom::new();
        let html = dom.create_element("html");
        let body = dom.create_element("body");
        let p = dom.create_element("p");
        dom.set_attr(p, "id", "greeting");
        dom.append_child(dom.root(), html);
        dom.append_child(html, body);
        dom.append_child(body, p);
        let before = dom.len();

        // One script exercises the whole tranche: getElementById + querySelector
        // (document and element) + createElement + appendChild + set/getAttribute,
        // all driving the arena DOM through the reflectors' reserved-slot NodeIds.
        let script = r#"
            var g = document.getElementById("greeting");
            var q = document.querySelector("p");
            var scoped = body_query();          // element.querySelector
            var parent = document.createElement("div");
            var child = document.createElement("span");
            parent.appendChild(child);
            parent.setAttribute("id", "made-in-js");
            (g !== null) && (q !== null) && (scoped !== null) &&
              (parent.getAttribute("id") === "made-in-js") &&
              (document.querySelector("span") === null)   // detached, not in tree
        "#;
        // `body_query` helper avoids relying on a `body` global.
        let script = format!(
            "function body_query() {{ return document.querySelector('body').querySelector('p'); }}\n{script}"
        );
        assert!(
            eval_bool(&mut dom, &script).expect("eval"),
            "DOM script mismatch"
        );
        // The two createElement calls grew the arena DOM.
        assert!(
            dom.len() >= before + 2,
            "createElement should grow the arena DOM: {} -> {}",
            before,
            dom.len()
        );
    }
}
