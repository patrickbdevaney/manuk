//! D3 **Step 0 prototype** — validate that our arena `manuk-dom` can back a
//! SpiderMonkey reflector via a reserved-slot `NodeId`, and measure the glue cost of
//! ONE interface before choosing hand-write vs Servo codegen.
//!
//! Modification boundary: this is the *sanctioned FFI/binding layer* — a `JSClass`
//! with reserved slots, `JS_NewObject`, `JS_DefineFunction`, and a `JSNative`. It
//! does **not** touch SpiderMonkey's JIT/GC internals or the sandbox.
//!
//! What it proves: an `Element` reflector stores `(NodeId, *const Dom)` in reserved
//! slots; a native reads them and returns `dom.text_content(node)` to JS — exactly
//! the mechanism Servo uses (Rust pointer in a reserved slot) but over our lean
//! arena DOM, no `Dom<T>`/`Reflector` machinery.

use std::ffi::c_char;
use std::ptr;

use mozjs::glue::JS_GetReservedSlot;
use mozjs::jsapi::{
    JSAutoRealm, JSClass, JSContext, JS_NewStringCopyN, JS_SetReservedSlot, OnNewGlobalHookOption,
    Value,
};
use mozjs::jsval::{ObjectValue, PrivateValue, StringValue, UndefinedValue};
use mozjs::rooted;
use mozjs::rust::wrappers2::{
    JS_DefineFunction, JS_DefineProperty, JS_NewGlobalObject, JS_NewObject,
};
use mozjs::rust::{RealmOptions, Runtime, SIMPLE_GLOBAL_CLASS};

use manuk_dom::{Dom, NodeId};

// Reserved-slot indices on the Element reflector.
const SLOT_NODE: u32 = 0;
const SLOT_DOM: u32 = 1;
const NUM_SLOTS: u32 = 2;
// JSClass flags encode the reserved-slot count in bits [8..16).
const RESERVED_SLOTS_SHIFT: u32 = 8;

static ELEMENT_CLASS: JSClass = JSClass {
    name: c"Element".as_ptr(),
    flags: NUM_SLOTS << RESERVED_SLOTS_SHIFT,
    cOps: ptr::null(),
    spec: ptr::null(),
    ext: ptr::null(),
    oOps: ptr::null(),
};

/// `textContentOf(element)` — reads the reflector's reserved slots and returns the
/// backing DOM node's text content. Uses the documented `CallArgsFromVp` `vp`
/// layout (`vp[0]` = return value, `vp[2..]` = args) to avoid the CallArgs wrapper.
unsafe extern "C" fn text_content_of(cx: *mut JSContext, argc: u32, vp: *mut Value) -> bool {
    if argc < 1 {
        *vp = UndefinedValue();
        return true;
    }
    let arg0 = *vp.add(2);
    if !arg0.is_object() {
        *vp = UndefinedValue();
        return true;
    }
    let obj = arg0.to_object();

    let mut node_slot = UndefinedValue();
    JS_GetReservedSlot(obj, SLOT_NODE, &mut node_slot);
    let mut dom_slot = UndefinedValue();
    JS_GetReservedSlot(obj, SLOT_DOM, &mut dom_slot);

    // Guard: if the reserved slots weren't populated, fail gracefully rather than
    // dereference garbage (turns a would-be segfault into a diagnosable `false`).
    if !node_slot.is_int32() {
        *vp = UndefinedValue();
        return true;
    }
    let dom_ptr = dom_slot.to_private() as *const Dom;
    if dom_ptr.is_null() {
        *vp = UndefinedValue();
        return true;
    }
    let node = NodeId(node_slot.to_int32() as usize);
    let dom = &*dom_ptr;
    let text = dom.text_content(node);

    let js_str = JS_NewStringCopyN(cx, text.as_ptr() as *const c_char, text.len());
    if js_str.is_null() {
        return false;
    }
    // vp[0] is the return slot (rooted on the VM stack by the caller).
    *vp = StringValue(&*js_str);
    true
}

/// Run the prototype: build an `Element` reflector over `(dom, node)`, expose it +
/// `textContentOf`, and evaluate `textContentOf(element) === <dom text>` in JS.
/// Returns whether JS agreed — proving the reflector's reserved-slot `NodeId`
/// correctly reaches the arena DOM. (Comparison is done JS-side to avoid reading a
/// JS string back through private glue.)
pub fn run(dom: &Dom, node: NodeId) -> Result<bool, String> {
    // Reuse the process-global engine (SpiderMonkey inits once per process).
    let handle = crate::spidermonkey::engine_handle().map_err(|e| e.message)?;
    let mut runtime = Runtime::new(handle);
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

    // Object allocation + property definition must happen inside the global's realm.
    let _ar = unsafe { JSAutoRealm::new(runtime.cx().raw_cx(), global.get()) };

    // Build the Element reflector and stash (NodeId, *const Dom) in reserved slots.
    rooted!(&in(runtime.cx()) let element = unsafe {
        JS_NewObject(runtime.cx(), &ELEMENT_CLASS)
    });
    if element.get().is_null() {
        return Err("JS_NewObject(Element) returned null".into());
    }
    unsafe {
        let node_val = mozjs::jsval::Int32Value(node.0 as i32);
        JS_SetReservedSlot(element.get(), SLOT_NODE, &node_val);
        let dom_val = PrivateValue(dom as *const Dom as *const std::ffi::c_void);
        JS_SetReservedSlot(element.get(), SLOT_DOM, &dom_val);
    }

    // Expose `element` and `textContentOf` on the global.
    rooted!(&in(runtime.cx()) let element_val = ObjectValue(element.get()));
    unsafe {
        JS_DefineProperty(
            runtime.cx(),
            global.handle(),
            c"element".as_ptr(),
            element_val.handle(),
            0,
        );
        JS_DefineFunction(
            runtime.cx(),
            global.handle(),
            c"textContentOf".as_ptr(),
            Some(text_content_of),
            1,
            0,
        );
    }

    // Compare JS-side against the Rust-computed text (avoids reading a JS string back).
    let expected = dom.text_content(node);
    let script = format!(
        "textContentOf(element) === {}",
        js_string_literal(&expected)
    );

    use mozjs::rust::{evaluate_script, CompileOptionsWrapper};
    rooted!(&in(runtime.cx()) let mut rval = UndefinedValue());
    let opts = CompileOptionsWrapper::new(runtime.cx_no_gc(), c"proto.js".to_owned(), 1);
    evaluate_script(
        runtime.cx(),
        global.handle(),
        &script,
        rval.handle_mut(),
        opts,
    )
    .map_err(|()| "evaluate_script failed".to_string())?;

    if !rval.get().is_boolean() {
        return Err("result was not a boolean".into());
    }
    Ok(rval.get().to_boolean())
}

/// Escape a Rust string as a JS double-quoted string literal.
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

#[cfg(test)]
mod tests {
    use super::*;

    // Run in isolation: SpiderMonkey does not cleanly support multiple `Runtime`
    // create/destroy cycles in one process, so this crashes at teardown if run in
    // the same binary as the other feature tests. Validate with:
    //   cargo test -p manuk-js --features spidermonkey bindings_prototype -- --ignored
    #[test]
    #[ignore = "SpiderMonkey multi-Runtime-per-process teardown; run in isolation"]
    fn element_text_content_over_arena_dom() {
        // Build a tiny arena DOM: <p>hello <b>world</b></p>
        let mut dom = Dom::new();
        let p = dom.create_element("p");
        let t1 = dom.create_text("hello ");
        let b = dom.create_element("b");
        let t2 = dom.create_text("world");
        dom.append_child(dom.root(), p);
        dom.append_child(p, t1);
        dom.append_child(p, b);
        dom.append_child(b, t2);

        assert_eq!(dom.text_content(p), "hello world");
        // JS `textContentOf(element)` — via the reflector's reserved-slot NodeId —
        // must equal the DOM's text content.
        assert!(
            run(&dom, p).expect("prototype run"),
            "JS textContent did not match the arena DOM"
        );
    }
}
