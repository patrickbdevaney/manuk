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
use mozjs::jsapi::{
    JSClass, JSContext as RawJSContext, JSObject, JS_SetReservedSlot, Value, JSPROP_ENUMERATE,
};
use mozjs::jsval::{
    BooleanValue, Int32Value, NullValue, ObjectValue, PrivateValue, UndefinedValue,
};
use mozjs::rooted;
use mozjs::rust::wrappers2::{
    CurrentGlobalOrNull, JS_DefineFunction, JS_DefineProperty, JS_DefineProperty1, JS_NewObject,
    JS_GetProperty, JS_NewGlobalObject, JS_SetElement1, JS_SetProperty, NewArrayObject1,
};
use mozjs::jsapi::OnNewGlobalHookOption;
use mozjs::gc::RootedTraceableBox;
use mozjs::jsapi::Heap;
use mozjs::rust::{evaluate_script, CompileOptionsWrapper, RealmOptions, Runtime, SIMPLE_GLOBAL_CLASS};

use manuk_dom::{Dom, NodeData, NodeId};

thread_local! {
    /// Pre-script layout snapshot (`NodeId` → `[x, y, width, height]`) exposed to
    /// `element.getBoundingClientRect()`. Set by [`run_scripts`] for the current document.
    static LAYOUT_RECTS: std::cell::RefCell<std::collections::HashMap<NodeId, [f32; 4]>> =
        std::cell::RefCell::new(std::collections::HashMap::new());

    /// Pre-script computed styles (`NodeId` → `ComputedStyle`) exposed to
    /// `getComputedStyle(el)`. Set by [`run_scripts`] for the current document.
    static STYLES: std::cell::RefCell<std::collections::HashMap<NodeId, manuk_css::ComputedStyle>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// A `Dim` as a CSS string.
fn dim_css(d: &manuk_css::Dim) -> String {
    match d {
        manuk_css::Dim::Auto => "auto".to_string(),
        manuk_css::Dim::Px(v) => format!("{v}px"),
        manuk_css::Dim::Percent(p) => format!("{p}%"),
        manuk_css::Dim::Calc { px, pct } => format!("calc({px}px + {pct}%)"),
    }
}

/// An `Rgba` as a CSS color string.
fn rgba_css(c: &manuk_css::Rgba) -> String {
    if c.a == 255 {
        format!("rgb({}, {}, {})", c.r, c.g, c.b)
    } else {
        format!("rgba({}, {}, {}, {})", c.r, c.g, c.b, c.a as f32 / 255.0)
    }
}

/// Serialize a `ComputedStyle` to a JS object-literal source (camelCase CSS properties →
/// CSS value strings) for `getComputedStyle`.
fn computed_style_js(cs: &manuk_css::ComputedStyle) -> String {
    use manuk_css::{Display, Overflow, Position, TextAlign};
    let display = match cs.display {
        Display::Block => "block",
        Display::Inline => "inline",
        Display::InlineBlock => "inline-block",
        Display::Flex => "flex",
        Display::Grid => "grid",
        Display::InlineFlex => "inline-flex",
        Display::InlineGrid => "inline-grid",
        Display::Table => "table",
        Display::TableRow => "table-row",
        Display::TableRowGroup => "table-row-group",
        Display::TableCell => "table-cell",
        Display::TableCaption => "table-caption",
        Display::TableColumn => "table-column",
        Display::TableColumnGroup => "table-column-group",
        Display::None => "none",
    };
    let position = match cs.position {
        Position::Static => "static",
        Position::Relative => "relative",
        Position::Absolute => "absolute",
        Position::Fixed => "fixed",
        Position::Sticky => "sticky",
    };
    let text_align = match cs.text_align {
        TextAlign::Left => "left",
        TextAlign::Center => "center",
        TextAlign::Right => "right",
        TextAlign::Justify => "justify",
    };
    // The computed `font-family` list, joined (its first entry is the primary).
    let family = cs.font_family.join(", ");
    let overflow = match cs.overflow {
        Overflow::Visible => "visible",
        Overflow::Hidden => "hidden",
        Overflow::Scroll => "scroll",
        Overflow::Auto => "auto",
        Overflow::Clip => "clip",
    };
    let q = js_string_literal;
    format!(
        "({{color:{}, backgroundColor:{}, fontSize:{}, fontWeight:{}, fontStyle:{}, \
          fontFamily:{}, lineHeight:{}, textAlign:{}, display:{}, position:{}, overflow:{}, \
          width:{}, height:{}, marginTop:{}, marginRight:{}, marginBottom:{}, marginLeft:{}, \
          paddingTop:{}, paddingRight:{}, paddingBottom:{}, paddingLeft:{}, \
          top:{}, right:{}, bottom:{}, left:{}, zIndex:{}, getPropertyValue:function(p){{\
          var m={{'background-color':'backgroundColor','font-size':'fontSize',\
          'font-weight':'fontWeight','font-style':'fontStyle','font-family':'fontFamily',\
          'line-height':'lineHeight','text-align':'textAlign','margin-top':'marginTop',\
          'margin-right':'marginRight','margin-bottom':'marginBottom','margin-left':'marginLeft',\
          'padding-top':'paddingTop','padding-right':'paddingRight','padding-bottom':'paddingBottom',\
          'padding-left':'paddingLeft','z-index':'zIndex'}};return this[m[p]||p];}}}})",
        q(&rgba_css(&cs.color)),
        q(&cs.background_color.map(|c| rgba_css(&c)).unwrap_or_else(|| "rgba(0, 0, 0, 0)".into())),
        q(&format!("{}px", cs.font_size)),
        q(&cs.font_weight.to_string()),
        q(if cs.italic { "italic" } else { "normal" }),
        q(&family),
        q(&format!("{}px", cs.line_height)),
        q(text_align),
        q(display),
        q(position),
        q(overflow),
        q(&dim_css(&cs.width)),
        q(&dim_css(&cs.height)),
        q(&dim_css(&cs.margin.top)),
        q(&dim_css(&cs.margin.right)),
        q(&dim_css(&cs.margin.bottom)),
        q(&dim_css(&cs.margin.left)),
        q(&dim_css(&cs.padding.top)),
        q(&dim_css(&cs.padding.right)),
        q(&dim_css(&cs.padding.bottom)),
        q(&dim_css(&cs.padding.left)),
        q(&dim_css(&cs.inset.top)),
        q(&dim_css(&cs.inset.right)),
        q(&dim_css(&cs.inset.bottom)),
        q(&dim_css(&cs.inset.left)),
        q(&cs.z_index.map(|z| z.to_string()).unwrap_or_else(|| "auto".into())),
    )
}

/// `getComputedStyle(element)` → a snapshot style object (camelCase props + a
/// `getPropertyValue("kebab-case")` accessor). Reads the pre-script computed styles.
unsafe extern "C" fn window_get_computed_style(
    cx: *mut RawJSContext,
    argc: u32,
    vp: *mut Value,
) -> bool {
    let node = arg_object(vp, argc, 0).and_then(|o| node_and_dom(o).map(|(_, n)| n));
    let js = node.and_then(|n| STYLES.with(|s| s.borrow().get(&n).map(computed_style_js)));
    let src = js.unwrap_or_else(|| "({})".to_string());
    match eval_in_current_global(cx, &src) {
        Some(v) => *vp = v,
        None => *vp = NullValue(),
    }
    true
}

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

/// Escape a Rust string as a JS double-quoted string literal (for embedding a value
/// into a script snippet).
fn js_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Evaluate `script` in the current global and return its value (or `None` on error).
/// Used by the event methods to drive the JS-side listener registry.
unsafe fn eval_in_current_global(cx: *mut RawJSContext, script: &str) -> Option<Value> {
    use mozjs::rust::{evaluate_script, CompileOptionsWrapper};
    let global = CurrentGlobalOrNull(&wrap_cx(cx));
    if global.is_null() {
        return None;
    }
    rooted!(in(cx) let g = global);
    rooted!(in(cx) let mut rval = UndefinedValue());
    let opts = CompileOptionsWrapper::new(&wrap_cx(cx), c"dom_event.js".to_owned(), 1);
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
    let id = node.0;
    // Identity cache: one wrapper per node, so `a.firstChild === b`, `event.target === el`
    // and the like hold (real sites rely on node identity). The cache is a JS-side
    // `__nodes` map, so its entries are GC-reachable through the global.
    if let Some(v) = eval_in_current_global(cx, &format!("(globalThis.__nodes&&__nodes[{id}])||null"))
    {
        if v.is_object() {
            return v.to_object();
        }
    }
    let obj_ptr = JS_NewObject(&mut wrap_cx(cx), &NODE_CLASS);
    rooted!(in(cx) let obj = obj_ptr);
    let node_val = Int32Value(node.0 as i32);
    JS_SetReservedSlot(obj.get(), SLOT_NODE, &node_val);
    let dom_val = PrivateValue(dom as *const std::ffi::c_void);
    JS_SetReservedSlot(obj.get(), SLOT_DOM, &dom_val);
    define_members(cx, &obj, false);
    // Store in the identity cache.
    let global = CurrentGlobalOrNull(&wrap_cx(cx));
    if !global.is_null() {
        rooted!(in(cx) let g = global);
        rooted!(in(cx) let ov = ObjectValue(obj.get()));
        JS_SetProperty(&mut wrap_cx(cx), g.handle(), c"__pending_node".as_ptr(), ov.handle());
        let _ = eval_in_current_global(
            cx,
            &format!(
                "(globalThis.__nodes||(globalThis.__nodes={{}}))[{id}]=__pending_node;\
                 __pending_node.__nodeId={id}"
            ),
        );
    }
    obj.get()
}

/// Emit a `MutationObserver` record to the JS-side pending list for delivery on the next
/// microtask checkpoint. `kind` is `"attributes"` / `"childList"` / `"characterData"`. Ensures
/// the target + added/removed nodes have reflectors (so `__nodes[id]` resolves in JS), then calls
/// `__recordMutation`. A no-op if `MutationObserver` was never touched (the pending machinery
/// still exists, so it just queues cheaply).
unsafe fn record_mutation(
    cx: *mut RawJSContext,
    dom: *mut Dom,
    kind: &str,
    target: NodeId,
    attr: Option<&str>,
    old_value: Option<&str>,
    added: &[NodeId],
    removed: &[NodeId],
) {
    // Reflect every node the record references so JS can resolve the ids back to node objects.
    let _ = new_reflector(cx, dom, target);
    for &n in added.iter().chain(removed.iter()) {
        let _ = new_reflector(cx, dom, n);
    }
    let ids = |v: &[NodeId]| {
        v.iter().map(|n| n.0.to_string()).collect::<Vec<_>>().join(",")
    };
    let attr_lit = attr.map(js_string_literal).unwrap_or_else(|| "null".to_string());
    let old_lit = old_value.map(js_string_literal).unwrap_or_else(|| "null".to_string());
    let script = format!(
        "if(globalThis.__recordMutation)__recordMutation({},{},{},{},{},{})",
        js_string_literal(kind),
        target.0,
        attr_lit,
        old_lit,
        js_string_literal(&ids(added)),
        js_string_literal(&ids(removed)),
    );
    let _ = eval_in_current_global(cx, &script);
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
    // Define an accessor property `name` with a `getter` and optional `setter`.
    let prop =
        |name: &std::ffi::CStr,
         getter: unsafe extern "C" fn(*mut RawJSContext, u32, *mut Value) -> bool,
         setter: Option<unsafe extern "C" fn(*mut RawJSContext, u32, *mut Value) -> bool>| {
            JS_DefineProperty1(
                &mut wrap_cx(cx),
                obj.handle(),
                name.as_ptr(),
                Some(getter),
                setter,
                JSPROP_ENUMERATE as u32,
            );
        };
    if is_document {
        def(c"getElementById", doc_get_by_id, 1);
        def(c"querySelector", doc_query, 1);
        def(c"querySelectorAll", doc_query_all, 1);
        def(c"createElement", doc_create_element, 1);
        def(c"createTextNode", doc_create_text_node, 1);
        def(c"getElementsByTagName", el_get_by_tag, 1);
        def(c"getElementsByClassName", el_get_by_class, 1);
        // Document-level (delegated) events.
        def(c"addEventListener", el_add_event_listener, 2);
        def(c"removeEventListener", el_remove_event_listener, 2);
        def(c"dispatchEvent", el_dispatch_event, 1);
    } else {
        def(c"appendChild", el_append_child, 1);
        def(c"setAttribute", el_set_attribute, 2);
        def(c"getAttribute", el_get_attribute, 1);
        def(c"removeAttribute", el_remove_attribute, 1);
        def(c"hasAttribute", el_has_attribute, 1);
        def(c"remove", el_remove, 0);
        def(c"attachShadow", el_attach_shadow, 1);
        def(c"getElementsByTagName", el_get_by_tag, 1);
        def(c"getElementsByClassName", el_get_by_class, 1);
        def(c"querySelector", doc_query, 1);
        def(c"querySelectorAll", doc_query_all, 1);
        def(c"addEventListener", el_add_event_listener, 2);
        def(c"removeEventListener", el_remove_event_listener, 2);
        def(c"dispatchEvent", el_dispatch_event, 1);
        def(c"getBoundingClientRect", el_get_bounding_rect, 0);
        // DOM tree mutation + cloning.
        def(c"insertBefore", el_insert_before, 2);
        def(c"removeChild", el_remove_child, 1);
        def(c"cloneNode", el_clone_node, 1);
        // DOM traversal (read-only accessor properties).
        prop(c"parentNode", el_get_parent_node, None);
        prop(c"shadowRoot", el_get_shadow_root, None);
        prop(c"parentElement", el_get_parent_element, None);
        prop(c"firstChild", el_get_first_child, None);
        prop(c"lastChild", el_get_last_child, None);
        prop(c"firstElementChild", el_get_first_element_child, None);
        prop(c"nextSibling", el_get_next_sibling, None);
        prop(c"previousSibling", el_get_prev_sibling, None);
        prop(c"nextElementSibling", el_get_next_element_sibling, None);
        prop(c"previousElementSibling", el_get_prev_element_sibling, None);
        prop(c"children", el_get_children, None);
        prop(c"childNodes", el_get_child_nodes, None);
        // Control IDL reflections.
        prop(c"value", el_get_value, Some(el_set_value));
        prop(c"checked", el_get_checked, Some(el_set_checked));
        // Accessor properties (jQuery-core read/write surface).
        prop(
            c"textContent",
            el_get_text_content,
            Some(el_set_text_content),
        );
        prop(c"innerHTML", el_get_inner_html, Some(el_set_inner_html));
        prop(c"tagName", el_get_tag_name, None); // read-only
        prop(c"id", el_get_id, Some(el_set_id));
        prop(c"className", el_get_class_name, Some(el_set_class_name));
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
unsafe extern "C" fn el_append_child(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((dom, parent)) = this_node(vp) else {
        *vp = UndefinedValue();
        return true;
    };
    match arg_object(vp, argc, 0).and_then(|o| node_and_dom(o).map(|(_, c)| (o, c))) {
        Some((child_obj, child)) => {
            (*dom).append_child(parent, child);
            record_mutation(cx, dom, "childList", parent, None, None, &[child], &[]);
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
        let old = (*dom).element(node).and_then(|e| e.attr(&name)).map(|s| s.to_string());
        record_mutation(cx, dom, "attributes", node, Some(&name), old.as_deref(), &[], &[]);
        (*dom).set_attr(node, name, value);
    }
    *vp = UndefinedValue();
    true
}

/// `element.getBoundingClientRect()` → a DOMRect-shaped object from the pre-script layout
/// snapshot (`{x, y, width, height, top, right, bottom, left}`), or all-zero if unlaid-out.
unsafe extern "C" fn el_get_bounding_rect(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    let node = this_node(vp).map(|(_, n)| n);
    let [x, y, w, h] = node
        .and_then(|n| LAYOUT_RECTS.with(|l| l.borrow().get(&n).copied()))
        .unwrap_or([0.0, 0.0, 0.0, 0.0]);
    let js = format!(
        "({{x:{x},y:{y},width:{w},height:{h},left:{x},top:{y},right:{r},bottom:{b}}})",
        r = x + w,
        b = y + h
    );
    match eval_in_current_global(cx, &js) {
        Some(v) => *vp = v,
        None => *vp = NullValue(),
    }
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

/// The next/previous sibling of `node` in `dom` skipping non-element nodes.
unsafe fn next_element(dom: *mut Dom, node: NodeId) -> Option<NodeId> {
    let mut cur = (*dom).next_sibling(node);
    while let Some(n) = cur {
        if (*dom).is_element(n) {
            return Some(n);
        }
        cur = (*dom).next_sibling(n);
    }
    None
}
unsafe fn prev_element(dom: *mut Dom, node: NodeId) -> Option<NodeId> {
    let mut cur = (*dom).prev_sibling(node);
    while let Some(n) = cur {
        if (*dom).is_element(n) {
            return Some(n);
        }
        cur = (*dom).prev_sibling(n);
    }
    None
}
unsafe fn first_element_child(dom: *mut Dom, node: NodeId) -> Option<NodeId> {
    let mut cur = (*dom).first_child(node);
    while let Some(n) = cur {
        if (*dom).is_element(n) {
            return Some(n);
        }
        cur = (*dom).next_sibling(n);
    }
    None
}

macro_rules! node_getter {
    ($name:ident, $f:expr) => {
        unsafe extern "C" fn $name(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
            match this_node(vp) {
                Some((dom, node)) => {
                    let f: fn(*mut Dom, NodeId) -> Option<NodeId> = $f;
                    return_node_or_null(cx, vp, dom, f(dom, node));
                }
                None => *vp = NullValue(),
            }
            true
        }
    };
}
node_getter!(el_get_parent_node, |dom, n| unsafe { (*dom).parent(n) });
node_getter!(el_get_parent_element, |dom, n| unsafe {
    (*dom).parent(n).filter(|&p| (*dom).is_element(p))
});
node_getter!(el_get_first_child, |dom, n| unsafe { (*dom).first_child(n) });
node_getter!(el_get_last_child, |dom, n| unsafe { (*dom).last_child(n) });
node_getter!(el_get_next_sibling, |dom, n| unsafe { (*dom).next_sibling(n) });
node_getter!(el_get_prev_sibling, |dom, n| unsafe { (*dom).prev_sibling(n) });
node_getter!(el_get_first_element_child, |d, n| unsafe { first_element_child(d, n) });
node_getter!(el_get_next_element_sibling, |d, n| unsafe { next_element(d, n) });
node_getter!(el_get_prev_element_sibling, |d, n| unsafe { prev_element(d, n) });

/// `element.children` — element children as a static Array.
unsafe extern "C" fn el_get_children(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    if let Some((dom, node)) = this_node(vp) {
        let kids: Vec<NodeId> = (*dom).children(node).filter(|&c| (*dom).is_element(c)).collect();
        node_array(cx, vp, dom, &kids);
    } else {
        *vp = NullValue();
    }
    true
}

/// `element.childNodes` — all child nodes as a static Array.
unsafe extern "C" fn el_get_child_nodes(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    if let Some((dom, node)) = this_node(vp) {
        let kids: Vec<NodeId> = (*dom).children(node).collect();
        node_array(cx, vp, dom, &kids);
    } else {
        *vp = NullValue();
    }
    true
}

/// `element.value` getter (form controls) — the `value` attribute, else empty string.
unsafe extern "C" fn el_get_value(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    let v = this_node(vp)
        .and_then(|(dom, n)| (*dom).element(n).and_then(|e| e.attr("value")).map(str::to_owned))
        .unwrap_or_default();
    return_string(cx, vp, &v);
    true
}
/// `element.value = s` setter.
unsafe extern "C" fn el_set_value(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    if let Some((dom, node)) = this_node(vp) {
        let v = arg_string(cx, vp, argc, 0).unwrap_or_default();
        (*dom).set_attr(node, "value", v);
    }
    *vp = UndefinedValue();
    true
}

/// `element.checked` getter — presence of the `checked` attribute.
unsafe extern "C" fn el_get_checked(_cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    let checked = this_node(vp)
        .and_then(|(dom, n)| (*dom).element(n).map(|e| e.attr("checked").is_some()))
        .unwrap_or(false);
    *vp = BooleanValue(checked);
    true
}
/// `element.checked = b` setter.
unsafe extern "C" fn el_set_checked(_cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    if let Some((dom, node)) = this_node(vp) {
        let on = argc > 0 && (*vp.add(2)).is_boolean() && (*vp.add(2)).to_boolean();
        if on {
            (*dom).set_attr(node, "checked", "");
        } else {
            (*dom).remove_attr(node, "checked");
        }
    }
    *vp = UndefinedValue();
    true
}

/// `parent.insertBefore(newChild, refChild)` — insert before `refChild` (or append if
/// `refChild` is null). Returns the inserted node.
unsafe extern "C" fn el_insert_before(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((dom, parent)) = this_node(vp) else {
        *vp = UndefinedValue();
        return true;
    };
    let new_child = arg_object(vp, argc, 0).and_then(|o| node_and_dom(o).map(|(_, n)| (o, n)));
    let reference = arg_object(vp, argc, 1).and_then(|o| node_and_dom(o).map(|(_, n)| n));
    match new_child {
        Some((obj, child)) => {
            match reference {
                Some(rf) => (*dom).insert_before(parent, child, rf),
                None => (*dom).append_child(parent, child),
            }
            record_mutation(cx, dom, "childList", parent, None, None, &[child], &[]);
            *vp = ObjectValue(obj);
        }
        None => *vp = NullValue(),
    }
    true
}

/// `parent.removeChild(child)` — detach `child`; returns it.
unsafe extern "C" fn el_remove_child(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((dom, parent)) = this_node(vp) else {
        *vp = UndefinedValue();
        return true;
    };
    match arg_object(vp, argc, 0).and_then(|o| node_and_dom(o).map(|(_, n)| (o, n))) {
        Some((obj, child)) => {
            record_mutation(cx, dom, "childList", parent, None, None, &[], &[child]);
            (*dom).remove_child(parent, child);
            *vp = ObjectValue(obj);
        }
        None => *vp = NullValue(),
    }
    true
}

/// `document.createTextNode(text)` → a detached text-node reflector.
unsafe extern "C" fn doc_create_text_node(
    cx: *mut RawJSContext,
    argc: u32,
    vp: *mut Value,
) -> bool {
    let Some((dom, _)) = this_node(vp) else {
        *vp = NullValue();
        return true;
    };
    let text = arg_string(cx, vp, argc, 0).unwrap_or_default();
    let node = (*dom).create_text(text);
    *vp = ObjectValue(new_reflector(cx, dom, node));
    true
}

/// Recursively clone `node` (elements copy tag+attrs; text copies content). `deep` clones
/// children too. Returns the new detached node.
unsafe fn clone_node(dom: *mut Dom, node: NodeId, deep: bool) -> NodeId {
    let new = match (*dom).data(node) {
        NodeData::Element(_) => {
            let tag = (*dom).tag_name(node).unwrap_or("div").to_string();
            let attrs: Vec<(String, String)> = (*dom)
                .element(node)
                .map(|e| {
                    e.attrs
                        .iter()
                        .map(|a| (a.name.clone(), a.value.clone()))
                        .collect()
                })
                .unwrap_or_default();
            let el = (*dom).create_element(tag);
            for (k, v) in attrs {
                (*dom).set_attr(el, k, v);
            }
            el
        }
        NodeData::Text(t) => (*dom).create_text(t.clone()),
        _ => (*dom).create_element("div"),
    };
    if deep {
        let kids: Vec<NodeId> = (*dom).children(node).collect();
        for k in kids {
            let ck = clone_node(dom, k, true);
            (*dom).append_child(new, ck);
        }
    }
    new
}

/// `node.cloneNode(deep)` → a detached clone.
unsafe extern "C" fn el_clone_node(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((dom, node)) = this_node(vp) else {
        *vp = NullValue();
        return true;
    };
    let deep = argc > 0 && (*vp.add(2)).is_boolean() && (*vp.add(2)).to_boolean();
    let clone = clone_node(dom, node, deep);
    *vp = ObjectValue(new_reflector(cx, dom, clone));
    true
}

/// `element.removeAttribute(name)`.
unsafe extern "C" fn el_remove_attribute(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    if let Some((dom, node)) = this_node(vp) {
        if let Some(name) = arg_string(cx, vp, argc, 0) {
            let old = (*dom).element(node).and_then(|e| e.attr(&name)).map(|s| s.to_string());
            record_mutation(cx, dom, "attributes", node, Some(&name), old.as_deref(), &[], &[]);
            (*dom).remove_attr(node, &name);
        }
    }
    *vp = UndefinedValue();
    true
}

/// `element.hasAttribute(name)` → boolean.
unsafe extern "C" fn el_has_attribute(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let has = match (this_node(vp), arg_string(cx, vp, argc, 0)) {
        (Some((dom, node)), Some(name)) => {
            (*dom).element(node).and_then(|e| e.attr(&name)).is_some()
        }
        _ => false,
    };
    *vp = BooleanValue(has);
    true
}

/// `element.remove()` — detach this node from its parent (DOM Living Standard `ChildNode`).
unsafe extern "C" fn el_remove(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    if let Some((dom, node)) = this_node(vp) {
        // Record on the parent (still attached) with this node as the removed child.
        if let Some(parent) = (*dom).parent(node) {
            record_mutation(cx, dom, "childList", parent, None, None, &[], &[node]);
        }
        (*dom).detach(node);
    }
    *vp = UndefinedValue();
    true
}

/// `element.attachShadow({mode})` — attach a shadow root to this element and return it as a
/// reflector (so `root.innerHTML = ...` / `root.appendChild(...)` work). The arena DOM already
/// models shadow roots as separate trees surfaced through the **flat tree**, so layout/paint pick
/// the content up with no further plumbing. Idempotent: a host that already has a shadow root
/// returns the existing one. `mode` defaults to `open`.
unsafe extern "C" fn el_attach_shadow(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((dom, host)) = this_node(vp) else {
        *vp = NullValue();
        return true;
    };
    // `{mode: "open"|"closed"}` — read the mode off the options object, defaulting to open.
    let mode = match arg_object(vp, argc, 0) {
        Some(opts) => {
            rooted!(in(cx) let o = opts);
            rooted!(in(cx) let mut v = UndefinedValue());
            let got = JS_GetProperty(&mut wrap_cx(cx), o.handle(), c"mode".as_ptr(), v.handle_mut());
            let is_closed = got && {
                let mut c = wrap_cx(cx);
                matches!(
                    String::safe_from_jsval(&mut c, v.handle(), ()),
                    Ok(ConversionResult::Success(ref s)) if s == "closed"
                )
            };
            if is_closed {
                manuk_dom::ShadowRootMode::Closed
            } else {
                manuk_dom::ShadowRootMode::Open
            }
        }
        None => manuk_dom::ShadowRootMode::Open,
    };
    let sr = (*dom).attach_shadow(host, mode);
    *vp = ObjectValue(new_reflector(cx, dom, sr));
    true
}

/// `element.shadowRoot` — the attached shadow root, or `null`. (An `closed` root is still
/// returned here; hiding it is a follow-on and would only obscure the page from itself.)
unsafe extern "C" fn el_get_shadow_root(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    match this_node(vp).and_then(|(dom, n)| (*dom).shadow_root(n).map(|sr| (dom, sr))) {
        Some((dom, sr)) => *vp = ObjectValue(new_reflector(cx, dom, sr)),
        None => *vp = NullValue(),
    }
    true
}

/// `getElementsByTagName(tag)` — live-ish collection (returned here as a static Array, like
/// `querySelectorAll`). `"*"` matches every descendant element. Installed on both document
/// and elements; delegates to the selector engine using the tag as a type selector.
unsafe extern "C" fn el_get_by_tag(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((dom, root)) = this_node(vp) else {
        *vp = NullValue();
        return true;
    };
    let tag = arg_string(cx, vp, argc, 0).unwrap_or_default();
    let matches = manuk_css::query_selector_all(&*dom, root, &tag);
    node_array(cx, vp, dom, &matches);
    true
}

/// `getElementsByClassName(name)` — descendants carrying every space-separated class in
/// `name`. Returned as a static Array. Delegates to the selector engine via a `.class`
/// compound selector.
unsafe extern "C" fn el_get_by_class(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((dom, root)) = this_node(vp) else {
        *vp = NullValue();
        return true;
    };
    let raw = arg_string(cx, vp, argc, 0).unwrap_or_default();
    // "a b" → ".a.b" (all classes must be present), matching the DOM semantics.
    let sel: String = raw
        .split_whitespace()
        .map(|c| format!(".{c}"))
        .collect();
    let matches = if sel.is_empty() {
        Vec::new()
    } else {
        manuk_css::query_selector_all(&*dom, root, &sel)
    };
    node_array(cx, vp, dom, &matches);
    true
}

/// Build a JS `Array` of element reflectors for `nodes` and store it in `*vp`.
unsafe fn node_array(cx: *mut RawJSContext, vp: *mut Value, dom: *mut Dom, nodes: &[NodeId]) {
    let arr_ptr = NewArrayObject1(&mut wrap_cx(cx), nodes.len());
    rooted!(in(cx) let arr = arr_ptr);
    for (i, &n) in nodes.iter().enumerate() {
        let refl = new_reflector(cx, dom, n);
        rooted!(in(cx) let robj = refl);
        JS_SetElement1(&mut wrap_cx(cx), arr.handle(), i as u32, robj.handle());
    }
    *vp = ObjectValue(arr.get());
}

/// `element.addEventListener(type, handler)` — register `handler` for `type` on this
/// node in the JS-side listener registry (keyed by the node's arena id). The handler
/// is stashed on the global, then a helper appends it — keeping it GC-rooted via the
/// registry. Requires [`install`]'s registry prelude.
unsafe extern "C" fn el_add_event_listener(
    cx: *mut RawJSContext,
    argc: u32,
    vp: *mut Value,
) -> bool {
    let Some((_, node)) = this_node(vp) else {
        *vp = UndefinedValue();
        return true;
    };
    let Some(ty) = arg_string(cx, vp, argc, 0) else {
        *vp = UndefinedValue();
        return true;
    };
    if argc < 2 {
        *vp = UndefinedValue();
        return true;
    }
    // Third arg: a boolean `true` selects the capture phase (an options object defaults to
    // bubble here; `{capture:true}` parsing is a follow-on).
    let capture = argc >= 3 && {
        let opt = *vp.add(4);
        opt.is_boolean() && opt.to_boolean()
    };
    // Stash the handler (arg 1) on the global, then register it via the helper.
    let global = CurrentGlobalOrNull(&wrap_cx(cx));
    if !global.is_null() {
        rooted!(in(cx) let g = global);
        rooted!(in(cx) let fnval = *vp.add(3));
        JS_SetProperty(
            &mut wrap_cx(cx),
            g.handle(),
            c"__pending_fn".as_ptr(),
            fnval.handle(),
        );
        let script = format!(
            "__addEventListener({}, {}, __pending_fn, {})",
            node.0,
            js_string_literal(&ty),
            capture
        );
        let _ = eval_in_current_global(cx, &script);
    }
    *vp = UndefinedValue();
    true
}

/// `element.removeEventListener(type, handler[, capture])`.
unsafe extern "C" fn el_remove_event_listener(
    cx: *mut RawJSContext,
    argc: u32,
    vp: *mut Value,
) -> bool {
    let Some((_, node)) = this_node(vp) else {
        *vp = UndefinedValue();
        return true;
    };
    let Some(ty) = arg_string(cx, vp, argc, 0) else {
        *vp = UndefinedValue();
        return true;
    };
    if argc < 2 {
        *vp = UndefinedValue();
        return true;
    }
    let capture = argc >= 3 && {
        let opt = *vp.add(4);
        opt.is_boolean() && opt.to_boolean()
    };
    let global = CurrentGlobalOrNull(&wrap_cx(cx));
    if !global.is_null() {
        rooted!(in(cx) let g = global);
        rooted!(in(cx) let fnval = *vp.add(3));
        JS_SetProperty(&mut wrap_cx(cx), g.handle(), c"__pending_fn".as_ptr(), fnval.handle());
        let script = format!(
            "__removeEventListener({}, {}, __pending_fn, {})",
            node.0,
            js_string_literal(&ty),
            capture
        );
        let _ = eval_in_current_global(cx, &script);
    }
    *vp = UndefinedValue();
    true
}

/// `element.dispatchEvent(type)` — synchronously invoke this node's listeners for
/// `type` (each gets an `{type}` event object). Returns whether any listener ran.
/// (Simplified: takes a type string rather than an `Event` object — no `Event`
/// constructor yet.) Runs synchronously, but can be *called from* a `setTimeout`
/// task, i.e. driven through the event loop.
unsafe extern "C" fn el_dispatch_event(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((_, node)) = this_node(vp) else {
        *vp = BooleanValue(false);
        return true;
    };
    let Some(ty) = arg_string(cx, vp, argc, 0) else {
        *vp = BooleanValue(false);
        return true;
    };
    let script = format!("__dispatchEvent({}, {})", node.0, js_string_literal(&ty));
    let ran = eval_in_current_global(cx, &script)
        .map(|v| v.is_boolean() && v.to_boolean())
        .unwrap_or(false);
    *vp = BooleanValue(ran);
    true
}

/// `document.querySelectorAll(sel)` / `element.querySelectorAll(sel)` → a JS `Array`
/// of element reflectors (a static NodeList, per this tranche).
unsafe extern "C" fn doc_query_all(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let Some((dom, root)) = this_node(vp) else {
        *vp = NullValue();
        return true;
    };
    let sel = arg_string(cx, vp, argc, 0).unwrap_or_default();
    let matches = manuk_css::query_selector_all(&*dom, root, &sel);

    let arr_ptr = NewArrayObject1(&mut wrap_cx(cx), matches.len());
    rooted!(in(cx) let arr = arr_ptr);
    for (i, &n) in matches.iter().enumerate() {
        let refl = new_reflector(cx, dom, n);
        rooted!(in(cx) let robj = refl);
        JS_SetElement1(&mut wrap_cx(cx), arr.handle(), i as u32, robj.handle());
    }
    *vp = ObjectValue(arr.get());
    true
}

// ---------------------------------------------------------------------------
// Element accessor properties (getters/setters)
// ---------------------------------------------------------------------------

/// `element.textContent` getter → the element's concatenated text.
unsafe extern "C" fn el_get_text_content(
    cx: *mut RawJSContext,
    _argc: u32,
    vp: *mut Value,
) -> bool {
    match this_node(vp) {
        Some((dom, node)) => {
            let text = (*dom).text_content(node);
            return_string(cx, vp, &text);
        }
        None => *vp = NullValue(),
    }
    true
}

/// `element.textContent = s` setter → replace all children with a single text node.
unsafe extern "C" fn el_set_text_content(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    if let Some((dom, node)) = this_node(vp) {
        let value = arg_string(cx, vp, argc, 0).unwrap_or_default();
        let kids: Vec<NodeId> = (*dom).children(node).collect();
        for &k in &kids {
            (*dom).detach(k);
        }
        let text = (*dom).create_text(value);
        (*dom).append_child(node, text);
        // childList: old children replaced by the single new text node.
        record_mutation(cx, dom, "childList", node, None, None, &[text], &kids);
    }
    *vp = UndefinedValue();
    true
}

/// `element.innerHTML` getter → the element's children serialized to HTML.
unsafe extern "C" fn el_get_inner_html(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    match this_node(vp) {
        Some((dom, node)) => {
            let html = manuk_html::serialize_inner(&*dom, node);
            return_string(cx, vp, &html);
        }
        None => *vp = NullValue(),
    }
    true
}

/// `element.innerHTML = s` setter → parse `s` and replace the element's children.
unsafe extern "C" fn el_set_inner_html(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    if let Some((dom, node)) = this_node(vp) {
        let value = arg_string(cx, vp, argc, 0).unwrap_or_default();
        let old_kids: Vec<NodeId> = (*dom).children(node).collect();
        manuk_html::set_inner_html(&mut *dom, node, &value);
        let new_kids: Vec<NodeId> = (*dom).children(node).collect();
        record_mutation(cx, dom, "childList", node, None, None, &new_kids, &old_kids);
    }
    *vp = UndefinedValue();
    true
}

/// `element.tagName` getter → the uppercase tag name (read-only, per DOM).
unsafe extern "C" fn el_get_tag_name(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    match this_node(vp) {
        Some((dom, node)) => match (*dom).tag_name(node) {
            Some(t) => {
                let upper = t.to_ascii_uppercase();
                return_string(cx, vp, &upper);
            }
            None => *vp = NullValue(),
        },
        None => *vp = NullValue(),
    }
    true
}

/// `element.id` getter → the `id` attribute (empty string if absent, per DOM).
unsafe extern "C" fn el_get_id(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    match this_node(vp) {
        Some((dom, node)) => {
            let id = (*dom).element(node).and_then(|e| e.id()).unwrap_or("");
            return_string(cx, vp, id);
        }
        None => *vp = NullValue(),
    }
    true
}

/// `element.id = s` setter.
unsafe extern "C" fn el_set_id(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    if let Some((dom, node)) = this_node(vp) {
        let v = arg_string(cx, vp, argc, 0).unwrap_or_default();
        (*dom).set_attr(node, "id", v);
    }
    *vp = UndefinedValue();
    true
}

/// `element.className` getter → the `class` attribute (empty string if absent).
unsafe extern "C" fn el_get_class_name(cx: *mut RawJSContext, _argc: u32, vp: *mut Value) -> bool {
    match this_node(vp) {
        Some((dom, node)) => {
            let cls = (*dom)
                .element(node)
                .and_then(|e| e.attr("class"))
                .unwrap_or("");
            return_string(cx, vp, cls);
        }
        None => *vp = NullValue(),
    }
    true
}

/// `element.className = s` setter.
unsafe extern "C" fn el_set_class_name(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    if let Some((dom, node)) = this_node(vp) {
        let v = arg_string(cx, vp, argc, 0).unwrap_or_default();
        (*dom).set_attr(node, "class", v);
    }
    *vp = UndefinedValue();
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
    doc_url: &str,
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

    // The JS-side event-listener registry that addEventListener/dispatchEvent drive.
    let _ = eval_in_current_global(cx, LISTENER_PRELUDE);
    // Seed the identity cache with the document (id = root) so event bubbling to
    // document-level (delegated) listeners resolves its node id.
    JS_SetProperty(
        &mut wrap_cx(cx),
        global.handle(),
        c"__pending_node".as_ptr(),
        doc_val.handle(),
    );
    let _ = eval_in_current_global(
        cx,
        &format!(
            "(globalThis.__nodes||(globalThis.__nodes={{}}))[{id}]=__pending_node;\
             document.__nodeId={id}",
            id = root.0
        ),
    );

    // Tier-0 BOM globals (window/self/console/navigator). Register the native console
    // sink, then install the JS shim with the honest UA + platform substituted in.
    JS_DefineFunction(
        &mut wrap_cx(cx),
        global.handle(),
        c"__hostLog".as_ptr(),
        Some(host_log),
        2,
        0,
    );
    JS_DefineFunction(
        &mut wrap_cx(cx),
        global.handle(),
        c"getComputedStyle".as_ptr(),
        Some(window_get_computed_style),
        1,
        0,
    );
    JS_DefineFunction(
        &mut wrap_cx(cx),
        global.handle(),
        c"__windowOpen".as_ptr(),
        Some(window_open),
        1,
        0,
    );
    JS_DefineFunction(
        &mut wrap_cx(cx),
        global.handle(),
        c"__historyPush".as_ptr(),
        Some(history_push),
        3,
        0,
    );
    JS_DefineFunction(
        &mut wrap_cx(cx),
        global.handle(),
        c"__postMessage".as_ptr(),
        Some(post_message),
        4,
        0,
    );
    let platform = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
    // JS string-literal-escape the document URL so it can't break out of the "%URL%" slot.
    let url_lit = {
        let esc = js_string_literal(doc_url); // yields a quoted literal
        esc[1..esc.len() - 1].to_string() // strip the quotes; %URL% sits inside "..."
    };
    let prelude = WINDOW_PRELUDE
        .replace("%UA%", &honest_user_agent())
        .replace("%PLATFORM%", &platform)
        .replace("%URL%", &url_lit);
    let _ = eval_in_current_global(cx, &prelude);
}

/// Run every inline `<script>` in `dom` against a **fresh global** in `runtime`, mutating the
/// arena DOM in place through the reflectors. Returns how many scripts executed. External
/// (`src`) scripts are skipped (network loading is the caller's concern), and a script that
/// throws is logged and the rest continue — exactly as a browser runs a page's scripts.
///
/// One global per document (the navigation model); the `Runtime` is reused across documents
/// by the caller (the process-global runtime), never re-created — that is what keeps
/// SpiderMonkey's single-Runtime-per-process rule.
pub fn run_scripts(
    runtime: &mut Runtime,
    dom: &mut Dom,
    layout: &std::collections::HashMap<NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<NodeId, manuk_css::ComputedStyle>,
) -> Result<usize, String> {
    let scripts = collect_inline_scripts(dom);
    if scripts.is_empty() {
        return Ok(0);
    }
    // Publish the pre-script layout + style snapshots for getBoundingClientRect /
    // getComputedStyle.
    LAYOUT_RECTS.with(|l| *l.borrow_mut() = layout.clone());
    STYLES.with(|s| *s.borrow_mut() = styles.clone());

    let options = RealmOptions::default();
    rooted!(&in(runtime.cx()) let global = unsafe {
        JS_NewGlobalObject(
            runtime.cx(),
            &SIMPLE_GLOBAL_CLASS,
            ptr::null_mut(),
            OnNewGlobalHookOption::FireOnNewGlobalHook,
            &*options,
        )
    });
    let raw_cx = unsafe { runtime.cx().raw_cx() };
    let _ar = mozjs::jsapi::JSAutoRealm::new(raw_cx, global.get());
    unsafe { install(raw_cx, &global, dom as *mut Dom, "") };
    crate::event_loop::install(runtime, global.handle())?;
    // Register the ES-module resolve hook (self-contained modules for now).
    unsafe {
        mozjs::jsapi::SetModuleResolveHook(
            mozjs::jsapi::JS_GetRuntime(raw_cx),
            Some(module_resolve_hook),
        );
    }

    let mut ran = 0usize;
    for (src, is_module) in &scripts {
        if *is_module {
            // `<script type=module>`: compile + link + evaluate as an ES module, so
            // import/export syntax is valid and self-contained modules run.
            if !unsafe { run_module(raw_cx, src) } {
                tracing::warn!("a page module failed (compile/link/evaluate); continuing");
            }
        } else {
            rooted!(&in(runtime.cx()) let mut rval = UndefinedValue());
            let opts = CompileOptionsWrapper::new(runtime.cx_no_gc(), c"inline.js".to_owned(), 1);
            match evaluate_script(runtime.cx(), global.handle(), src, rval.handle_mut(), opts) {
                Ok(()) => {}
                Err(()) => tracing::warn!("a page <script> threw; continuing with the rest"),
            }
        }
        ran += 1;
    }

    // Drain microtasks (Promise reactions) and macrotasks (setTimeout) the scripts queued.
    crate::event_loop::run(runtime, global.handle())?;
    Ok(ran)
}

/// A persistent per-document JS context — the keystone of an *interactive* page.
///
/// [`run_scripts`] creates a throwaway global, so every event listener a page registers is
/// destroyed the instant load finishes: a later click has nothing to fire. `PageContext`
/// instead keeps the document's global alive in a [`RootedTraceableBox`] (GC-rooted across
/// event-loop turns), so listeners registered while the page's scripts run survive to fire on
/// real user input via [`dispatch`](PageContext::dispatch).
///
/// # Lifetime / safety contract
/// The `Dom` passed to [`load`](PageContext::load) and every [`dispatch`](PageContext::dispatch)
/// must be the **same live `Dom` at a stable address** for the context's lifetime — reflectors
/// cache a raw `*mut Dom`. A navigation builds a fresh `PageContext` over the new document.
pub struct PageContext {
    global: RootedTraceableBox<Heap<*mut JSObject>>,
}

impl PageContext {
    /// Build the persistent global, install the DOM+BOM bindings and the event loop, then run
    /// the document's inline scripts (registering their listeners). Returns the context to keep
    /// alive alongside the page, and the number of scripts that ran.
    pub fn load(
        runtime: &mut Runtime,
        dom: &mut Dom,
        doc_url: &str,
        layout: &std::collections::HashMap<NodeId, [f32; 4]>,
        styles: &std::collections::HashMap<NodeId, manuk_css::ComputedStyle>,
    ) -> Result<(Self, usize), String> {
        LAYOUT_RECTS.with(|l| *l.borrow_mut() = layout.clone());
        STYLES.with(|s| *s.borrow_mut() = styles.clone());

        let options = RealmOptions::default();
        rooted!(&in(runtime.cx()) let global = unsafe {
            JS_NewGlobalObject(runtime.cx(), &SIMPLE_GLOBAL_CLASS, ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook, &*options)
        });
        let raw_cx = unsafe { runtime.cx().raw_cx() };
        let _ar = mozjs::jsapi::JSAutoRealm::new(raw_cx, global.get());
        unsafe { install(raw_cx, &global, dom as *mut Dom, doc_url) };
        crate::event_loop::install(runtime, global.handle())?;
        unsafe {
            mozjs::jsapi::SetModuleResolveHook(
                mozjs::jsapi::JS_GetRuntime(raw_cx),
                Some(module_resolve_hook),
            );
        }

        let mut ran = 0usize;
        for (src, is_module) in collect_inline_scripts(dom) {
            if is_module {
                if !unsafe { run_module(raw_cx, &src) } {
                    tracing::warn!("a page module failed (compile/link/evaluate); continuing");
                }
            } else {
                rooted!(&in(runtime.cx()) let mut rval = UndefinedValue());
                let opts = CompileOptionsWrapper::new(runtime.cx_no_gc(), c"inline.js".to_owned(), 1);
                if evaluate_script(runtime.cx(), global.handle(), &src, rval.handle_mut(), opts).is_err() {
                    tracing::warn!("a page <script> threw; continuing with the rest");
                }
            }
            ran += 1;
        }
        // Deferred: load-time fetch/XHR stays queued for the host to perform (see run_deferred);
        // resolving inline would settle every request with status 0 (no real network here).
        crate::event_loop::run_deferred(runtime, global.handle())?;

        // Promote the stack-rooted global to a persistent root so it outlives this call.
        let boxed = RootedTraceableBox::new(Heap::default());
        boxed.set(global.get());
        Ok((Self { global: boxed }, ran))
    }

    /// Dispatch a trusted `ty` event (e.g. `"click"`, `"input"`) to `node`, running its
    /// listeners (and delegated listeners up the ancestor chain) synchronously plus any
    /// microtasks/timers they queue. Returns `true` if the engine should still perform the
    /// element's **default action** (navigation, submit) — i.e. no listener called
    /// `preventDefault()`.
    pub fn dispatch(
        &self,
        runtime: &mut Runtime,
        dom: &mut Dom,
        node: NodeId,
        ty: &str,
        layout: &std::collections::HashMap<NodeId, [f32; 4]>,
        styles: &std::collections::HashMap<NodeId, manuk_css::ComputedStyle>,
    ) -> Result<bool, String> {
        LAYOUT_RECTS.with(|l| *l.borrow_mut() = layout.clone());
        STYLES.with(|s| *s.borrow_mut() = styles.clone());

        let raw_cx = unsafe { runtime.cx().raw_cx() };
        rooted!(&in(runtime.cx()) let global = self.global.get());
        let _ar = mozjs::jsapi::JSAutoRealm::new(raw_cx, global.get());
        // Reflect the target node (idempotent) so it is registered in `__nodes` and
        // `__dispatchEvent` can resolve it and walk its ancestor chain for delegation.
        unsafe {
            let _ = new_reflector(raw_cx, dom as *mut Dom, node);
        }
        let script = format!("__dispatchEvent({}, {})", node.0, js_string_literal(ty));
        rooted!(&in(runtime.cx()) let mut rval = UndefinedValue());
        let opts = CompileOptionsWrapper::new(runtime.cx_no_gc(), c"dispatch.js".to_owned(), 1);
        let proceed = match evaluate_script(runtime.cx(), global.handle(), &script, rval.handle_mut(), opts) {
            // `__dispatchEvent` returns `!defaultPrevented`; a non-boolean or error means
            // nothing suppressed the default, so proceed.
            Ok(()) => {
                let v = rval.get();
                !v.is_boolean() || v.to_boolean()
            }
            Err(()) => true,
        };
        // Deferred: a handler's fetch/XHR stays queued for the host (pumped after dispatch).
        crate::event_loop::run_deferred(runtime, global.handle())?;
        Ok(proceed)
    }

    /// Drain this document's queued `fetch`/XHR requests as `(id, url, method, body)` so the
    /// host can perform them over the real network and settle each via [`resolve_fetch`].
    pub fn take_fetches(
        &self,
        runtime: &mut Runtime,
    ) -> Result<Vec<(u32, String, String, String)>, String> {
        let raw_cx = unsafe { runtime.cx().raw_cx() };
        rooted!(&in(runtime.cx()) let global = self.global.get());
        let _ar = mozjs::jsapi::JSAutoRealm::new(raw_cx, global.get());
        crate::event_loop::drain_pending(runtime, global.handle())
    }

    /// Drain this document's queued `history` ops as `(kind, state_json, url)` (see
    /// [`take_pending_history`]). Host-side thread-local, so no realm entry is needed.
    pub fn take_history_ops(&self) -> Vec<(u8, String, String)> {
        take_pending_history()
    }

    /// Fire a `popstate` event into this document's window (real back/forward), updating
    /// `history.state` + `location` first, then running the page's `onpopstate`/listeners and
    /// any DOM mutations they make. `state_json` is a JSON string (`"null"` for no state);
    /// `url` (if non-empty) becomes the new `location`.
    pub fn fire_popstate(
        &self,
        runtime: &mut Runtime,
        dom: &mut Dom,
        state_json: &str,
        url: &str,
        layout: &std::collections::HashMap<NodeId, [f32; 4]>,
        styles: &std::collections::HashMap<NodeId, manuk_css::ComputedStyle>,
    ) -> Result<(), String> {
        LAYOUT_RECTS.with(|l| *l.borrow_mut() = layout.clone());
        STYLES.with(|s| *s.borrow_mut() = styles.clone());
        let _ = dom;

        let raw_cx = unsafe { runtime.cx().raw_cx() };
        rooted!(&in(runtime.cx()) let global = self.global.get());
        let _ar = mozjs::jsapi::JSAutoRealm::new(raw_cx, global.get());
        let set_url = if url.is_empty() {
            String::new()
        } else {
            format!("globalThis.__applyUrl({});", js_string_literal(url))
        };
        let script = format!(
            "(function(){{ var s = JSON.parse({}); globalThis.__histState = s; {} \
             globalThis.__fireWindowEvent('popstate', {{type:'popstate', state:s}}); }})()",
            js_string_literal(state_json),
            set_url
        );
        rooted!(&in(runtime.cx()) let mut rval = UndefinedValue());
        let opts = CompileOptionsWrapper::new(runtime.cx_no_gc(), c"popstate.js".to_owned(), 1);
        if evaluate_script(runtime.cx(), global.handle(), &script, rval.handle_mut(), opts).is_err() {
            tracing::warn!("popstate dispatch threw; continuing");
        }
        crate::event_loop::run_deferred(runtime, global.handle())?;
        Ok(())
    }

    /// Seed this document's window **identity** after load: its own window id (stamped as the
    /// `source` on messages it posts) and its opener's id (`window.opener`, `0` = none). Called
    /// by the host once the tab's id linkage is known.
    pub fn set_identity(&self, runtime: &mut Runtime, win_id: u64, opener_win: u64) -> Result<(), String> {
        let raw_cx = unsafe { runtime.cx().raw_cx() };
        rooted!(&in(runtime.cx()) let global = self.global.get());
        let _ar = mozjs::jsapi::JSAutoRealm::new(raw_cx, global.get());
        let script = format!(
            "globalThis.__winId = {win_id}; globalThis.opener = {};",
            if opener_win == 0 {
                "null".to_string()
            } else {
                format!("globalThis.__makeWindowRef({opener_win})")
            }
        );
        rooted!(&in(runtime.cx()) let mut rval = UndefinedValue());
        let opts = CompileOptionsWrapper::new(runtime.cx_no_gc(), c"identity.js".to_owned(), 1);
        let _ = evaluate_script(runtime.cx(), global.handle(), &script, rval.handle_mut(), opts);
        Ok(())
    }

    /// Drain this document's queued `postMessage` sends as `(target_win, json, origin,
    /// source_win)` so the host can route each to the destination window's [`deliver_message`].
    pub fn take_messages(&self) -> Vec<(u64, String, String, u64)> {
        take_pending_messages()
    }

    /// Deliver a cross-window message: fire a `message` `MessageEvent` (`{data, origin, source}`)
    /// on this document's window, then run the handler's reactions. `origin` is the sender's
    /// origin (the receiver may check it); `source_win` (`0` = none) lets the handler reply via
    /// `event.source.postMessage`.
    pub fn deliver_message(
        &self,
        runtime: &mut Runtime,
        dom: &mut Dom,
        json: &str,
        origin: &str,
        source_win: u64,
        layout: &std::collections::HashMap<NodeId, [f32; 4]>,
        styles: &std::collections::HashMap<NodeId, manuk_css::ComputedStyle>,
    ) -> Result<(), String> {
        LAYOUT_RECTS.with(|l| *l.borrow_mut() = layout.clone());
        STYLES.with(|s| *s.borrow_mut() = styles.clone());
        let _ = dom;

        let raw_cx = unsafe { runtime.cx().raw_cx() };
        rooted!(&in(runtime.cx()) let global = self.global.get());
        let _ar = mozjs::jsapi::JSAutoRealm::new(raw_cx, global.get());
        let script = format!(
            "globalThis.__deliverMessage({}, {}, {})",
            js_string_literal(json),
            js_string_literal(origin),
            source_win
        );
        rooted!(&in(runtime.cx()) let mut rval = UndefinedValue());
        let opts = CompileOptionsWrapper::new(runtime.cx_no_gc(), c"message.js".to_owned(), 1);
        if evaluate_script(runtime.cx(), global.handle(), &script, rval.handle_mut(), opts).is_err() {
            tracing::warn!("message delivery threw; continuing");
        }
        crate::event_loop::run_deferred(runtime, global.handle())?;
        Ok(())
    }

    /// Settle a pending `fetch`/`XHR` request (issued earlier via the `__fetch` host queue) by
    /// id: evaluates `__resolveFetch(id, status, body)` in this document's persistent global,
    /// which resolves the stored Promise / drives the XHR callbacks, then drains the event loop
    /// so the page's `.then(...)` / `onload` reactions (and any DOM mutations they make) run.
    /// `status == 0` signals a network failure (rejects the Promise / fires `onerror`).
    pub fn resolve_fetch(
        &self,
        runtime: &mut Runtime,
        dom: &mut Dom,
        id: u32,
        status: u16,
        body: &str,
        layout: &std::collections::HashMap<NodeId, [f32; 4]>,
        styles: &std::collections::HashMap<NodeId, manuk_css::ComputedStyle>,
    ) -> Result<(), String> {
        LAYOUT_RECTS.with(|l| *l.borrow_mut() = layout.clone());
        STYLES.with(|s| *s.borrow_mut() = styles.clone());
        let _ = dom; // reflectors cache the stable `*mut Dom` from `load`; kept for API symmetry.

        let raw_cx = unsafe { runtime.cx().raw_cx() };
        rooted!(&in(runtime.cx()) let global = self.global.get());
        let _ar = mozjs::jsapi::JSAutoRealm::new(raw_cx, global.get());
        crate::event_loop::deliver(runtime, global.handle(), id, status, body)?;
        // Run the reactions (`.then` / `onload`) and any DOM mutations they make; a follow-on
        // fetch they issue stays queued for the host's next pump.
        crate::event_loop::run_deferred(runtime, global.handle())?;
        Ok(())
    }
}

/// Compile + link + evaluate `source` as an ES module in the current realm. Returns false
/// if any stage fails. A module with imports that the resolve hook can't satisfy fails at
/// link; self-contained modules (no imports, `export`, `import.meta`, top-level await) run.
unsafe fn run_module(cx: *mut RawJSContext, source: &str) -> bool {
    use mozjs::jsapi::{CompileModule, ModuleEvaluate, ModuleLink};
    let opts = CompileOptionsWrapper::new(&wrap_cx(cx), c"module.js".to_owned(), 1);
    let utf16: Vec<u16> = source.encode_utf16().collect();
    let mut src = mozjs::rust::transform_u16_to_source_text(&utf16);
    let module = CompileModule(cx, opts.ptr, &mut src);
    if module.is_null() {
        return false;
    }
    rooted!(in(cx) let mod_obj = module);
    if !ModuleLink(cx, mod_obj.handle().into()) {
        return false;
    }
    rooted!(in(cx) let mut rval = UndefinedValue());
    ModuleEvaluate(cx, mod_obj.handle().into(), rval.handle_mut().into())
}

/// ES-module resolve hook. Self-contained modules only for now: imports resolve to null,
/// so `ModuleLink` fails for a module that imports (caught by the caller). A registry of
/// pre-fetched modules keyed by resolved specifier is the follow-on for import graphs.
unsafe extern "C" fn module_resolve_hook(
    _cx: *mut RawJSContext,
    _referencing: mozjs::jsapi::JS::Handle<mozjs::jsapi::Value>,
    _request: mozjs::jsapi::JS::Handle<*mut JSObject>,
) -> *mut JSObject {
    ptr::null_mut()
}

/// The inline JavaScript sources of a document, in tree order. Skips `src=` scripts and
/// non-JS `type`s (e.g. `application/json`).
fn collect_inline_scripts(dom: &Dom) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for n in dom.descendants(dom.root()) {
        if dom.tag_name(n) != Some("script") {
            continue;
        }
        let mut is_module = false;
        if let Some(el) = dom.element(n) {
            if el.attr("src").is_some() {
                continue;
            }
            let ty = el.attr("type").unwrap_or("").trim();
            is_module = ty.eq_ignore_ascii_case("module");
            let is_js = ty.is_empty()
                || ty.eq_ignore_ascii_case("text/javascript")
                || ty.eq_ignore_ascii_case("application/javascript")
                || is_module;
            if !is_js {
                continue;
            }
        }
        let src = dom.text_content(n);
        if !src.trim().is_empty() {
            out.push((src, is_module));
        }
    }
    out
}

/// The listener registry + helpers backing `addEventListener`/`dispatchEvent`.
/// Listeners are keyed by `"<nodeId>:<type>"` and kept GC-alive via the global
/// `__listeners` map.
const LISTENER_PRELUDE: &str = r#"
    globalThis.__listeners = {};
    globalThis.__addEventListener = function(nid, type, fn, capture) {
        if (typeof fn !== 'function') return;
        var k = nid + ':' + type + ':' + (capture ? 'c' : 'b');
        (__listeners[k] || (__listeners[k] = [])).push(fn);
    };
    globalThis.__removeEventListener = function(nid, type, fn, capture) {
        var k = nid + ':' + type + ':' + (capture ? 'c' : 'b');
        var arr = __listeners[k];
        if (!arr) return;
        var i = arr.indexOf(fn);
        if (i >= 0) arr.splice(i, 1);
    };
    // A real Event with capture/bubble propagation, target/currentTarget, preventDefault
    // and stopPropagation. Returns false iff preventDefault was called (so the engine can
    // decide whether to run the default action).
    globalThis.__dispatchEvent = function(nid, type) {
        var target = (globalThis.__nodes && __nodes[nid]) || null;
        // Ancestor path: target, parent, ... root.
        var path = [];
        for (var cur = target; cur; cur = cur.parentNode) path.push(cur);
        var ev = {
            type: type, target: target, currentTarget: null, eventPhase: 0,
            bubbles: true, cancelable: true, defaultPrevented: false, _stop: false,
            preventDefault: function () { this.defaultPrevented = true; },
            stopPropagation: function () { this._stop = true; },
            stopImmediatePropagation: function () { this._stop = true; }
        };
        var invoke = function (node, phase) {
            if (!node || ev._stop) return;
            var arr = __listeners[node.__nodeId + ':' + type + ':' + phase];
            if (!arr) return;
            ev.currentTarget = node;
            for (var i = 0; i < arr.length && !ev._stop; i++) {
                try { arr[i].call(node, ev); } catch (e) {}
            }
        };
        // Capture: root → target's parent.
        ev.eventPhase = 1;
        for (var i = path.length - 1; i >= 1; i--) invoke(path[i], 'c');
        // At target (both capture- and bubble-registered).
        ev.eventPhase = 2;
        invoke(path[0], 'c'); invoke(path[0], 'b');
        // Bubble: target's parent → root.
        ev.eventPhase = 3;
        if (ev.bubbles) for (var i = 1; i < path.length; i++) invoke(path[i], 'b');
        return !ev.defaultPrevented;
    };
"#;

/// Tier-0 window/BOM globals. A modern bundle caches `window`/`self`/`console`/`navigator`
/// eagerly at load; a single missing one is a `ReferenceError` that aborts the whole
/// `<script>` before any DOM API is reached. Defining them (self-referential `window`,
/// a `console` that routes to the host log, an honest `navigator`) converts that whole
/// class of instant aborts into "runs". `%UA%`/`%PLATFORM%` are substituted at install.
const WINDOW_PRELUDE: &str = r#"
    (function () {
        var g = globalThis;
        if (typeof g.window === 'undefined') g.window = g;
        if (typeof g.self === 'undefined') g.self = g;
        var mk = function (level) {
            return function () {
                var parts = [];
                for (var i = 0; i < arguments.length; i++) {
                    try { parts.push(String(arguments[i])); } catch (e) { parts.push('?'); }
                }
                try { g.__hostLog(level, parts.join(' ')); } catch (e) {}
            };
        };
        g.console = g.console || {};
        var methods = ['log','info','debug','warn','error','trace','dir','table',
                       'group','groupCollapsed','groupEnd','assert','count','time','timeEnd'];
        for (var i = 0; i < methods.length; i++) {
            var m = methods[i];
            if (typeof g.console[m] !== 'function') {
                g.console[m] = mk(m === 'error' ? 'error' : (m === 'warn' ? 'warn' : 'log'));
            }
        }
        g.navigator = g.navigator || {
            userAgent: "%UA%",
            appName: "Netscape", appCodeName: "Mozilla", appVersion: "5.0",
            product: "Gecko", platform: "%PLATFORM%",
            language: "en-US", languages: ["en-US", "en"],
            onLine: true, cookieEnabled: false, doNotTrack: null
        };
        // window.open → the host opens a real tab/window (OAuth-popup pattern). Returns a
        // stub window handle so `var w = window.open(...)` and `w.close()` work.
        if (typeof g.open !== 'function') {
            g.open = function (u) {
                try { return g.__windowOpen(String(u == null ? '' : u)); } catch (e) { return null; }
            };
        }
        // Viewport / screen metrics. Real sites (and every SPA framework) read these at boot and
        // throw a ReferenceError if absent. Honest, ordinary-human desktop values (not spoofed to
        // mimic a specific competitor); a follow-on threads the true window size through.
        var VW = 1280, VH = 720;
        if (typeof g.innerWidth === 'undefined') g.innerWidth = VW;
        if (typeof g.innerHeight === 'undefined') g.innerHeight = VH;
        if (typeof g.outerWidth === 'undefined') g.outerWidth = VW;
        if (typeof g.outerHeight === 'undefined') g.outerHeight = VH;
        if (typeof g.devicePixelRatio === 'undefined') g.devicePixelRatio = 1;
        if (typeof g.screenX === 'undefined') g.screenX = 0;
        if (typeof g.screenY === 'undefined') g.screenY = 0;
        if (typeof g.screen === 'undefined') g.screen = {
            width: VW, height: VH, availWidth: VW, availHeight: VH,
            colorDepth: 24, pixelDepth: 24, orientation: { type: 'landscape-primary', angle: 0 }
        };
        if (typeof g.matchMedia === 'undefined') {
            // Evaluate the common media features against the viewport (VW×VH), mirroring the CSS
            // @media cascade, so JS responsive branches agree with the rendered layout. Commas =
            // OR, ` and ` = AND; unknown features don't block (evaluate true).
            g.__evalMediaFeature = function (f) {
                f = String(f).trim();
                var m = f.match(/^\(?\s*([a-z-]+)\s*(?::\s*([^)]+))?\)?$/);
                if (!m) return f.indexOf('print') < 0;
                var name = m[1], val = (m[2] || '').trim(), px = parseFloat(val);
                switch (name) {
                    case 'min-width': return VW >= px;
                    case 'max-width': return VW <= px;
                    case 'min-height': return VH >= px;
                    case 'max-height': return VH <= px;
                    case 'width': return VW === px;
                    case 'height': return VH === px;
                    case 'orientation': return (val === 'landscape') === (VW >= VH);
                    case 'prefers-color-scheme': return val === 'light';
                    case 'prefers-reduced-motion': return val !== 'reduce';
                    case 'screen': case 'all': return true;
                    case 'print': return false;
                    default: return true;
                }
            };
            g.__evalMedia = function (q) {
                q = String(q).toLowerCase().trim();
                var ors = q.split(',');
                for (var i = 0; i < ors.length; i++) {
                    var parts = ors[i].split(' and '), ok = true;
                    for (var j = 0; j < parts.length; j++) {
                        if (!g.__evalMediaFeature(parts[j])) { ok = false; break; }
                    }
                    if (ok) return true;
                }
                return false;
            };
            g.matchMedia = function (q) {
                return { matches: g.__evalMedia(q), media: String(q), onchange: null,
                         addListener: function () {}, removeListener: function () {},
                         addEventListener: function () {}, removeEventListener: function () {},
                         dispatchEvent: function () { return false; } };
            };
        }
        if (typeof g.requestAnimationFrame === 'undefined') {
            g.requestAnimationFrame = function (cb) { return setTimeout(function () { cb(Date.now()); }, 16); };
            g.cancelAnimationFrame = function (id) { clearTimeout(id); };
        }
        // fetch / XMLHttpRequest are installed by the event-loop prelude (see event_loop.rs),
        // which owns the host request queue + delivery.

        // window-level events (popstate/load/...) — a small registry separate from the node
        // listener map, since window is not an arena node.
        if (typeof g.__winListeners === 'undefined') {
            g.__winListeners = {};
            var _origAdd = g.addEventListener;
            g.addEventListener = function (type, fn, capture) {
                if (typeof fn === 'function') (g.__winListeners[type] = g.__winListeners[type] || []).push(fn);
            };
            g.removeEventListener = function (type, fn) {
                var a = g.__winListeners[type]; if (!a) return;
                var i = a.indexOf(fn); if (i >= 0) a.splice(i, 1);
            };
            g.__fireWindowEvent = function (type, ev) {
                var a = (g.__winListeners[type] || []).slice();
                for (var i = 0; i < a.length; i++) { try { a[i].call(g, ev); } catch (e) {} }
                var on = g['on' + type];
                if (typeof on === 'function') { try { on.call(g, ev); } catch (e) {} }
            };
        }

        // location + history — client-side (SPA) routing. pushState/replaceState update
        // location and queue an op for the host (__historyPush) to reflect in the omnibox +
        // back/forward stack WITHOUT a network navigation. The host fires popstate on real
        // back/forward via __fireWindowEvent('popstate', ...).
        g.__parseUrl = function (href) {
            var m = /^([a-zA-Z][a-zA-Z0-9+.\-]*:)\/\/([^\/?#]*)([^?#]*)(\?[^#]*)?(#.*)?$/.exec(href);
            if (!m) { m = [href, 'https:', '', href.charAt(0) === '/' ? href : '/' + href, '', '']; }
            var protocol = m[1] || 'https:', host = m[2] || '', path = m[3] || '/';
            var hostParts = host.split(':');
            return {
                href: href, protocol: protocol, host: host,
                hostname: hostParts[0] || '', port: hostParts[1] || '',
                pathname: path || '/', search: m[4] || '', hash: m[5] || '',
                origin: protocol + '//' + host,
                assign: function (u) { g.__applyUrl(String(u)); },
                replace: function (u) { g.__applyUrl(String(u)); },
                reload: function () {}, toString: function () { return this.href; }
            };
        };
        g.__applyUrl = function (u) {
            u = String(u);
            var loc = g.location, abs;
            if (/^[a-zA-Z][a-zA-Z0-9+.\-]*:\/\//.test(u)) abs = u;
            else if (u.charAt(0) === '?') abs = loc.origin + loc.pathname + u;
            else if (u.charAt(0) === '#') abs = loc.origin + loc.pathname + loc.search + u;
            else if (u.charAt(0) === '/') abs = loc.origin + u;
            else abs = loc.origin + loc.pathname.replace(/[^\/]*$/, '') + u;
            g.location = g.__parseUrl(abs);
        };
        if (typeof g.location === 'undefined' || typeof g.location.pathname === 'undefined') {
            g.location = g.__parseUrl("%URL%");
        }
        g.__histState = (typeof g.__histState === 'undefined') ? null : g.__histState;
        if (typeof g.history === 'undefined' || typeof g.history.pushState !== 'function') {
            var _len = 1;
            g.history = {
                get state() { return g.__histState; },
                get length() { return _len; },
                get scrollRestoration() { return 'auto'; },
                set scrollRestoration(v) {},
                pushState: function (st, title, url) {
                    g.__histState = (st == null ? null : st);
                    if (url != null) { g.__applyUrl(String(url)); _len++; }
                    try { g.__historyPush('0', JSON.stringify(g.__histState), g.location.href); } catch (e) {}
                },
                replaceState: function (st, title, url) {
                    g.__histState = (st == null ? null : st);
                    if (url != null) g.__applyUrl(String(url));
                    try { g.__historyPush('1', JSON.stringify(g.__histState), g.location.href); } catch (e) {}
                },
                back: function () { try { g.__historyPush('2', 'null', ''); } catch (e) {} },
                forward: function () { try { g.__historyPush('3', 'null', ''); } catch (e) {} },
                go: function (n) { try { g.__historyPush('4', 'null', String(n == null ? 0 : n)); } catch (e) {} }
            };
        }

        // Cross-window messaging (postMessage / opener). Each document has a window id
        // (`__winId`, seeded by the host after load); a window *handle* (opener, or the value
        // window.open returns) is a small ref carrying the target's id. postMessage routes the
        // (structured-clone-lite / JSON) payload through the host to that window, which fires a
        // `message` MessageEvent on the receiver via the window event registry (built for
        // popstate). This completes the OAuth-popup round-trip begun by window.open.
        if (typeof g.__winId === 'undefined') g.__winId = 0;
        g.__makeWindowRef = function (winId) {
            return {
                __winId: winId, closed: false,
                postMessage: function (msg, targetOrigin) {
                    var json;
                    try { json = JSON.stringify(msg === undefined ? null : msg); } catch (e) { json = 'null'; }
                    try {
                        g.__postMessage(String(winId), json,
                            String(targetOrigin == null ? '*' : targetOrigin), String(g.__winId || 0));
                    } catch (e) {}
                },
                close: function () { this.closed = true; }, focus: function () {}, blur: function () {}
            };
        };
        if (typeof g.opener === 'undefined') g.opener = null;
        // Host → receiver: build a MessageEvent and fire it. `sourceWin` (0 = none) lets the
        // handler reply via `event.source.postMessage(...)`.
        g.__deliverMessage = function (json, origin, sourceWin) {
            var data; try { data = JSON.parse(json); } catch (e) { data = null; }
            var ev = {
                type: 'message', data: data, origin: String(origin || ''),
                source: sourceWin ? g.__makeWindowRef(sourceWin) : null,
                ports: [], lastEventId: ''
            };
            g.__fireWindowEvent('message', ev);
        };

        // Custom elements. `HTMLElement` is the base every custom element extends. On *upgrade*
        // we set `__ceUnderConstruction` to the element being upgraded; `super()` then reaches
        // this constructor, which RETURNS that element — and per ES semantics a derived
        // constructor's `this` becomes the object its base constructor returned. So the author's
        // `constructor() { super(); this.attachShadow(...) }` runs with `this` === the real
        // element, exactly as the spec's upgrade does.
        if (typeof g.HTMLElement === 'undefined') {
            g.__ceUnderConstruction = null;
            g.HTMLElement = function HTMLElement() {
                if (g.__ceUnderConstruction) return g.__ceUnderConstruction;
                return this;
            };
            g.HTMLElement.prototype = {};
        }
        if (typeof g.customElements === 'undefined') {
            g.customElements = {
                __defs: {},
                define: function (name, ctor) {
                    name = String(name).toLowerCase();
                    if (this.__defs[name]) return; // already defined
                    this.__defs[name] = ctor;
                    g.__upgradeTag(name);
                },
                get: function (name) { return this.__defs[String(name).toLowerCase()]; },
                whenDefined: function (name) {
                    return Promise.resolve(this.__defs[String(name).toLowerCase()]);
                }
            };
            // Upgrade one element: graft the class's prototype methods onto the host object (a
            // reflector's prototype can't be swapped), run the constructor with `this` bound to
            // it, then fire the lifecycle callbacks.
            g.__upgradeEl = function (el, ctor) {
                if (!el || el.__ceUpgraded) return;
                el.__ceUpgraded = true;
                var proto = ctor && ctor.prototype;
                if (proto) {
                    var keys = Object.getOwnPropertyNames(proto);
                    for (var i = 0; i < keys.length; i++) {
                        var k = keys[i];
                        if (k === 'constructor') continue;
                        try { el[k] = proto[k]; } catch (e) {}
                    }
                }
                var prev = g.__ceUnderConstruction;
                g.__ceUnderConstruction = el;
                try { new ctor(); } catch (e) { try { g.__hostLog('warn', 'custom element ctor: ' + e); } catch (x) {} }
                g.__ceUnderConstruction = prev;
                // attributeChangedCallback for observed attributes already present.
                var obs = ctor && ctor.observedAttributes;
                if (obs && obs.length && typeof el.attributeChangedCallback === 'function') {
                    for (var j = 0; j < obs.length; j++) {
                        var a = obs[j], v = el.getAttribute(a);
                        if (v !== null) {
                            try { el.attributeChangedCallback(a, null, v); } catch (e) {}
                        }
                    }
                }
                // It is already in the document (we only scan the live tree), so connect it.
                if (typeof el.connectedCallback === 'function') {
                    try { el.connectedCallback(); } catch (e) {}
                }
            };
            g.__upgradeTag = function (name) {
                var ctor = g.customElements.__defs[name];
                if (!ctor) return;
                var els = document.getElementsByTagName(name) || [];
                for (var i = 0; i < els.length; i++) g.__upgradeEl(els[i], ctor);
            };
            // Sweep every defined tag — run after DOM mutations so elements inserted later
            // (the common SPA pattern) are upgraded too.
            g.__upgradeScan = function () {
                for (var name in g.customElements.__defs) {
                    if (Object.prototype.hasOwnProperty.call(g.customElements.__defs, name)) {
                        g.__upgradeTag(name);
                    }
                }
            };
        }

        // MutationObserver. The native DOM-mutating methods emit records via __recordMutation;
        // matching records are delivered to each observer as a microtask (after the current
        // script), exactly as the spec batches them. SPAs mutate the DOM post-fetch and watch it
        // this way; absent the API their code throws at construction.
        if (typeof g.MutationObserver === 'undefined') {
            g.__moObservers = [];
            g.__pendingMutations = [];
            g.__moScheduled = false;
            g.__nodeById = function (id) { return (g.__nodes && g.__nodes[id]) || null; };
            g.__moListToNodes = function (csv) {
                if (!csv) return [];
                var out = [], parts = String(csv).split(',');
                for (var i = 0; i < parts.length; i++) {
                    if (parts[i] === '') continue;
                    var n = g.__nodeById(parts[i]); if (n) out.push(n);
                }
                return out;
            };
            g.__recordMutation = function (type, targetId, attrName, oldValue, addedCsv, removedCsv) {
                g.__pendingMutations.push({
                    type: type, targetId: targetId, attrName: attrName,
                    oldValue: oldValue, addedCsv: addedCsv, removedCsv: removedCsv
                });
                if (!g.__moScheduled) { g.__moScheduled = true; queueMicrotask(g.__deliverMutations); }
            };
            g.__moIsDescendant = function (node, ancestor) {
                for (var cur = node && node.parentNode; cur; cur = cur.parentNode) {
                    if (cur === ancestor) return true;
                }
                return false;
            };
            g.__moMatches = function (t, rec, target) {
                var o = t.options || {};
                if (rec.type === 'attributes' && !o.attributes) return false;
                if (rec.type === 'childList' && !o.childList) return false;
                if (rec.type === 'characterData' && !o.characterData) return false;
                if (rec.type === 'attributes' && o.attributeFilter &&
                    o.attributeFilter.indexOf(rec.attrName) < 0) return false;
                if (target === t.node) return true;
                if (o.subtree && g.__moIsDescendant(target, t.node)) return true;
                return false;
            };
            g.__buildRecord = function (rec) {
                var target = g.__nodeById(rec.targetId);
                var attrs = (rec.type === 'attributes');
                var chars = (rec.type === 'characterData');
                return {
                    type: rec.type, target: target,
                    addedNodes: g.__moListToNodes(rec.addedCsv),
                    removedNodes: g.__moListToNodes(rec.removedCsv),
                    previousSibling: null, nextSibling: null,
                    attributeName: attrs ? rec.attrName : null, attributeNamespace: null,
                    oldValue: (attrs || chars) ? rec.oldValue : null
                };
            };
            g.__deliverMutations = function () {
                g.__moScheduled = false;
                var recs = g.__pendingMutations; g.__pendingMutations = [];
                // Elements inserted since the last checkpoint may be custom elements awaiting
                // upgrade (the common SPA pattern: render markup, then the component boots).
                if (g.__upgradeScan) { try { g.__upgradeScan(); } catch (e) {} }
                if (!recs.length) return;
                for (var i = 0; i < g.__moObservers.length; i++) {
                    var obs = g.__moObservers[i];
                    var matched = [];
                    for (var j = 0; j < recs.length; j++) {
                        var rec = recs[j];
                        var target = g.__nodeById(rec.targetId);
                        if (!target) continue;
                        for (var k = 0; k < obs.__targets.length; k++) {
                            if (g.__moMatches(obs.__targets[k], rec, target)) {
                                matched.push(g.__buildRecord(rec)); break;
                            }
                        }
                    }
                    if (matched.length && typeof obs.__cb === 'function') {
                        try { obs.__cb(matched, obs); } catch (e) {}
                    }
                }
            };
            g.MutationObserver = function (cb) { this.__cb = cb; this.__targets = []; this.__records = []; };
            g.MutationObserver.prototype.observe = function (target, options) {
                if (!target) return;
                this.__targets.push({ node: target, options: options || {} });
                if (g.__moObservers.indexOf(this) < 0) g.__moObservers.push(this);
            };
            g.MutationObserver.prototype.disconnect = function () {
                this.__targets = [];
                var i = g.__moObservers.indexOf(this); if (i >= 0) g.__moObservers.splice(i, 1);
            };
            g.MutationObserver.prototype.takeRecords = function () {
                var r = this.__records; this.__records = []; return r;
            };
        }
    })();
"#;

/// `__hostLog(level, message)` — native sink behind the `console.*` shim; routes page
/// logs to `tracing` (stderr) so they surface instead of vanishing, and so a page that
/// calls `console.log` neither throws nor is silently dropped.
unsafe extern "C" fn host_log(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let level = arg_string(cx, vp, argc, 0).unwrap_or_else(|| "log".to_string());
    let msg = arg_string(cx, vp, argc, 1).unwrap_or_default();
    match level.as_str() {
        "error" => tracing::error!(target: "page.console", "{msg}"),
        "warn" => tracing::warn!(target: "page.console", "{msg}"),
        _ => tracing::info!(target: "page.console", "{msg}"),
    }
    *vp = UndefinedValue();
    true
}

/// The honest `navigator.userAgent` — same form as the network `User-Agent` (Axis F:
/// truthful, never competitor mimicry), built locally to avoid a dep on `manuk-net`.
fn honest_user_agent() -> String {
    let os = match std::env::consts::OS {
        "linux" => "X11; Linux",
        "macos" => "Macintosh; macOS",
        "windows" => "Windows NT",
        other => other,
    };
    format!(
        "Mozilla/5.0 ({}; {}) Manuk/{} (+standards)",
        os,
        std::env::consts::ARCH,
        env!("CARGO_PKG_VERSION")
    )
}

thread_local! {
    /// `window.open(...)` requests since the last drain: `(win_id, url)`. The host opens a real
    /// tab/window and records `win_id → tab` so a later `postMessage` to the returned handle
    /// routes to it (the OAuth-popup pattern).
    static PENDING_OPENS: std::cell::RefCell<Vec<(u64, String)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Monotonic window-id source, shared with the host via [`next_window_id`] so ids allocated
    /// by `window.open` never collide with ids the host assigns to ordinary tabs.
    static WIN_ID: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// `postMessage` sends since the last drain: `(target_win, json, origin, source_win)`.
    static PENDING_MESSAGES: std::cell::RefCell<Vec<(u64, String, String, u64)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Allocate the next process-unique window id (host side, for ordinary tabs).
pub fn next_window_id() -> u64 {
    WIN_ID.with(|c| {
        let next = c.get().wrapping_add(1);
        c.set(next);
        next
    })
}

/// `window.open` requests since the last drain, each `(win_id, url)` (host side).
pub fn take_pending_window_opens() -> Vec<(u64, String)> {
    PENDING_OPENS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// `postMessage` sends since the last drain, each `(target_win, json, origin, source_win)`.
pub fn take_pending_messages() -> Vec<(u64, String, String, u64)> {
    PENDING_MESSAGES.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// `window.open(url, ...)` — allocate a window id, record `(win_id, url)` for the host to open
/// as a new tab/window, and return a window **handle** carrying that id (so `w = window.open()`,
/// `w.postMessage(...)`, `w.closed`, `w.close()` all work and route to the opened window).
unsafe extern "C" fn window_open(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let url = arg_string(cx, vp, argc, 0).unwrap_or_default();
    let win = next_window_id();
    if !url.is_empty() {
        PENDING_OPENS.with(|q| q.borrow_mut().push((win, url)));
    }
    match eval_in_current_global(cx, &format!("globalThis.__makeWindowRef({win})")) {
        Some(v) => *vp = v,
        None => *vp = NullValue(),
    }
    true
}

/// `__postMessage(targetWin, json, origin, sourceWin)` — queue a cross-window message for the
/// host to route to the target window's document (which fires a `message` MessageEvent).
unsafe extern "C" fn post_message(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let target: u64 = arg_string(cx, vp, argc, 0)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let json = arg_string(cx, vp, argc, 1).unwrap_or_else(|| "null".to_string());
    let origin = arg_string(cx, vp, argc, 2).unwrap_or_else(|| "*".to_string());
    let source: u64 = arg_string(cx, vp, argc, 3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if target != 0 {
        PENDING_MESSAGES.with(|q| q.borrow_mut().push((target, json, origin, source)));
    }
    *vp = UndefinedValue();
    true
}

thread_local! {
    /// `history` operations the page performed since the last drain, each
    /// `(kind, state_json, url)` where kind: 0=pushState 1=replaceState 2=back 3=forward
    /// 4=go(n) (url holds n). The host reflects these in the omnibox + its back/forward stack
    /// WITHOUT a network navigation (SPA client-side routing).
    static PENDING_HISTORY: std::cell::RefCell<Vec<(u8, String, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// `history` ops the page performed since the last drain (host side).
pub fn take_pending_history() -> Vec<(u8, String, String)> {
    PENDING_HISTORY.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// `__historyPush(kind, stateJson, url)` — record a `history` op for the host. `kind` arrives
/// as a stringified small integer (see [`PENDING_HISTORY`]).
unsafe extern "C" fn history_push(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    let kind: u8 = arg_string(cx, vp, argc, 0)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let state = arg_string(cx, vp, argc, 1).unwrap_or_else(|| "null".to_string());
    let url = arg_string(cx, vp, argc, 2).unwrap_or_default();
    PENDING_HISTORY.with(|q| q.borrow_mut().push((kind, state, url)));
    *vp = UndefinedValue();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use mozjs::jsapi::OnNewGlobalHookOption;
    use mozjs::rust::wrappers2::JS_NewGlobalObject;
    use mozjs::rust::{
        evaluate_script, CompileOptionsWrapper, RealmOptions, Runtime, SIMPLE_GLOBAL_CLASS,
    };

    /// Evaluate `script`, returning its boolean value, against a global with `dom`,
    /// the event loop, and (if `run_loop`) a drained event loop afterward.
    fn eval_scene(dom: &mut Dom, setup: &str, run_loop: bool, check: &str) -> Result<bool, String> {
        let handle = crate::spidermonkey::engine_handle().map_err(|e| e.message)?;
        let mut runtime = Runtime::new(handle);
        let options = RealmOptions::default();
        rooted!(&in(runtime.cx()) let global = unsafe {
            JS_NewGlobalObject(runtime.cx(), &SIMPLE_GLOBAL_CLASS, ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook, &*options)
        });
        let raw_cx = unsafe { runtime.cx().raw_cx() };
        let _ar = mozjs::jsapi::JSAutoRealm::new(raw_cx, global.get());
        unsafe { install(raw_cx, &global, dom as *mut Dom, "https://test.example/") };
        crate::event_loop::install(&mut runtime, global.handle())?;

        eval_val(&mut runtime, global.handle(), setup)?;
        if run_loop {
            crate::event_loop::run(&mut runtime, global.handle())?;
        }
        let r = eval_val(&mut runtime, global.handle(), check)?;
        Ok(r.is_boolean() && r.to_boolean())
    }

    /// Evaluate `src` in `global`, returning its value.
    fn eval_val(
        rt: &mut Runtime,
        global: mozjs::rust::HandleObject,
        src: &str,
    ) -> Result<mozjs::jsapi::Value, String> {
        rooted!(&in(rt.cx()) let mut rval = UndefinedValue());
        let opts = CompileOptionsWrapper::new(rt.cx_no_gc(), c"dom.js".to_owned(), 1);
        evaluate_script(rt.cx(), global, src, rval.handle_mut(), opts)
            .map_err(|()| "evaluate_script failed".to_string())?;
        Ok(rval.get())
    }

    // `run_scripts` is verified end-to-end through the render pipeline (a real HTML page
    // whose inline script builds content renders that content) — see the shell's
    // spidermonkey render path. A manual-DOM unit test here would need its own Runtime,
    // which collides with `dom_methods_over_arena` (one Runtime per process) and adds no
    // coverage the E2E path lacks.

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

        // One script exercises the whole surface so far: methods (getElementById /
        // querySelector / querySelectorAll / createElement / appendChild /
        // get-setAttribute) AND the new accessor properties (textContent get+set,
        // tagName read-only, id get+set, className get+set) — all driving the arena
        // DOM through the reflectors' reserved-slot NodeIds.
        let script = r#"
            var g = document.getElementById("greeting");
            var scoped = body_query();          // element.querySelector
            var parent = document.createElement("div");
            var child = document.createElement("span");
            parent.appendChild(child);

            g.textContent = "hello world";      // accessor setter → arena mutation
            parent.id = "made-in-js";
            parent.className = "box active";
            parent.setAttribute("data-k", "v");

            // innerHTML: parse a fragment into the element, then read it back.
            parent.innerHTML = "<em>hi</em><i>yo</i>";
            var inner_ok = (parent.innerHTML === "<em>hi</em><i>yo</i>") &&
                           (parent.querySelectorAll("em").length === 1) &&
                           (parent.textContent === "hiyo");

            var all = document.querySelectorAll("p");   // NodeList (JS Array)

            // Collections + attribute-presence + removal (ChildNode.remove).
            var col = document.createElement("div");
            var c1 = document.createElement("b"); c1.className = "hit x";
            var c2 = document.createElement("b"); c2.className = "hit";
            var c3 = document.createElement("i"); c3.className = "hit";
            col.appendChild(c1); col.appendChild(c2); col.appendChild(c3);
            var byTag   = col.getElementsByTagName("b").length === 2;
            var byStar  = col.getElementsByTagName("*").length === 3;
            var byClass = col.getElementsByClassName("hit").length === 3;
            var byBoth  = col.getElementsByClassName("hit x").length === 1;
            c2.remove();                                  // detach middle <b>
            var afterRemove = col.getElementsByTagName("b").length === 1;
            c1.setAttribute("k", "v");
            var attrPresent = c1.hasAttribute("k") === true && c1.hasAttribute("nope") === false;
            c1.removeAttribute("k");
            var attrGone = c1.hasAttribute("k") === false;
            var newApis = byTag && byStar && byClass && byBoth &&
                          afterRemove && attrPresent && attrGone;

            // Tier-0 BOM globals: window/self identity, a callable console (must not
            // throw), and an honest navigator.userAgent.
            console.log("bom probe", 1, {a: 2});   // must not throw
            // Traversal + mutation + control IDL + cloning.
            var ul = document.createElement("ul");
            var li1 = document.createElement("li");
            var li2 = document.createElement("li");
            ul.appendChild(li1); ul.appendChild(li2);
            var travOk = (ul.firstChild === li1) && (ul.lastChild === li2) &&
                         (li1.nextSibling === li2) && (li2.previousSibling === li1) &&
                         (li1.parentNode === ul) && (li1.parentElement === ul) &&
                         (ul.children.length === 2) && (ul.childNodes.length === 2);
            var li0 = document.createElement("li");
            ul.insertBefore(li0, li1);
            var insOk = (ul.firstChild === li0) && (ul.children.length === 3);
            ul.removeChild(li0);
            var remOk = (ul.children.length === 2) && (ul.firstChild === li1);
            li1.appendChild(document.createTextNode("hello"));
            var textOk = (li1.textContent === "hello");
            var clone = ul.cloneNode(true);
            var cloneOk = (clone.children.length === 2) && (clone !== ul);
            var inp = document.createElement("input");
            inp.value = "typed"; inp.checked = true;
            var ctrlOk = (inp.value === "typed") && (inp.checked === true) &&
                         (inp.getAttribute("value") === "typed");
            inp.checked = false;
            var domApis2 = travOk && insOk && remOk && textOk && cloneOk && ctrlOk &&
                           (inp.checked === false);

            var bomOk = (window === globalThis) && (self === globalThis) &&
                        (typeof console.log === 'function') &&
                        (typeof navigator.userAgent === 'string') &&
                        (navigator.userAgent.indexOf('Manuk') >= 0) &&
                        (navigator.language === 'en-US');

            // Events: register a listener, dispatch synchronously, and schedule a
            // dispatch through the event loop.
            globalThis.clicks = 0;
            var btn = document.createElement("button");
            btn.addEventListener("click", function(ev){ if (ev.type === "click") clicks += 1; });
            var immediate = btn.dispatchEvent("click");   // sync → clicks = 1, returns true
            var noListener = btn.dispatchEvent("hover");   // no listener → not prevented → true

            // Capture → target → bubble ordering, event.target, and preventDefault.
            var outer = document.createElement("div");
            var inner = document.createElement("span");
            outer.appendChild(inner);
            globalThis.evlog = [];
            outer.addEventListener("click", function(ev){ evlog.push("bubble:" + (ev.target === inner)); });
            outer.addEventListener("click", function(ev){ evlog.push("capture"); }, true);
            inner.addEventListener("click", function(ev){ evlog.push("target"); ev.preventDefault(); });
            var notPrevented = inner.dispatchEvent("click");
            var evOk = (evlog.join(",") === "capture,target,bubble:true") && (notPrevented === false);
            setTimeout(function(){ btn.dispatchEvent("click"); });  // via the loop → clicks = 2

            globalThis.dom_ok =
              (g !== null) && (scoped !== null) && inner_ok &&
              (g.textContent === "hello world") &&
              (g.tagName === "P") && (parent.tagName === "DIV") &&
              (g.id === "greeting") && (parent.id === "made-in-js") &&
              (parent.className === "box active") &&
              (parent.getAttribute("data-k") === "v") &&
              (Array.isArray(all)) && (all.length === 1) && (all[0].tagName === "P") &&
              newApis && bomOk && domApis2 &&
              (document.querySelector("span") === null) &&  // detached, not in tree
              (immediate === true) && (noListener === true) && (clicks === 1) && evOk;
        "#;
        // `body_query` helper avoids relying on a `body` global.
        let setup = format!(
            "function body_query() {{ return document.querySelector('body').querySelector('p'); }}\n{script}"
        );
        // After the loop runs, the scheduled dispatch has fired → clicks === 2.
        let ok = eval_scene(&mut dom, &setup, true, "dom_ok && clicks === 2").expect("eval");
        assert!(ok, "DOM + events scene mismatch");
        // The textContent setter wrote a real text node into the arena DOM.
        assert_eq!(dom.text_content(p), "hello world");
        // createElement + the text node grew the arena DOM.
        assert!(
            dom.len() >= before + 2,
            "createElement should grow the arena DOM: {} -> {}",
            before,
            dom.len()
        );
    }
}
