//! SpiderMonkey-backed [`JsRuntime`], compiled only under `--features spidermonkey`.
//!
//! This is the sanctioned integration surface: we create an engine + runtime, make
//! a global with a realm, and evaluate script via the high-level `mozjs::rust`
//! wrappers (`evaluate_script`, `CompileOptionsWrapper`). We do **not** touch
//! JIT/GC internals or the sandbox — see the crate docs and CLAUDE.md §
//! modification boundary. The shape here follows mozjs's own `examples/eval.rs`.

use std::ffi::CString;
use std::ptr;
use std::sync::OnceLock;

use mozjs::jsapi::OnNewGlobalHookOption;
use mozjs::jsval::UndefinedValue;
use mozjs::rooted;
use mozjs::rust::wrappers2::*;
use mozjs::rust::SIMPLE_GLOBAL_CLASS;
use mozjs::rust::{
    evaluate_script, CompileOptionsWrapper, JSEngine, JSEngineHandle, RealmOptions, Runtime,
};

use crate::{JsError, JsRuntime, JsValue};

/// The process-global SpiderMonkey engine. `JSEngine::init()` may run only once per
/// process, so we initialize it exactly once (via `OnceLock`) and share the handle
/// across every runtime/thread — the standard embedding pattern. The engine is
/// deliberately leaked so it lives for the whole process.
static ENGINE: OnceLock<Option<JSEngineHandle>> = OnceLock::new();

pub(crate) fn engine_handle() -> Result<JSEngineHandle, JsError> {
    ENGINE
        .get_or_init(|| match JSEngine::init() {
            Ok(engine) => {
                let handle = engine.handle();
                std::mem::forget(engine); // keep the engine alive for the process
                Some(handle)
            }
            Err(_) => None,
        })
        .clone()
        .ok_or_else(|| JsError {
            message: "SpiderMonkey JSEngine::init() failed".to_string(),
        })
}

/// A SpiderMonkey runtime bound to the current thread.
///
/// SpiderMonkey is thread-affine, so this type is intentionally not `Send`. It
/// borrows the process-global engine ([`engine_handle`]); many runtimes (e.g. one
/// isolate per tab) share that single engine.
pub struct SpiderMonkeyRuntime {
    runtime: Runtime,
}

impl SpiderMonkeyRuntime {
    pub fn new() -> Result<Self, JsError> {
        let runtime = Runtime::new(engine_handle()?);
        Ok(SpiderMonkeyRuntime { runtime })
    }
}

impl JsRuntime for SpiderMonkeyRuntime {
    fn eval(&mut self, source: &str, filename: &str) -> Result<JsValue, JsError> {
        let options = RealmOptions::default();
        // Fresh global/realm per eval keeps this simple; a persistent global is
        // where the DOM bindings (crate::bindings) would live.
        rooted!(&in(self.runtime.cx()) let global = unsafe {
            JS_NewGlobalObject(
                self.runtime.cx(),
                &SIMPLE_GLOBAL_CLASS,
                ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook,
                &*options,
            )
        });

        rooted!(&in(self.runtime.cx()) let mut rval = UndefinedValue());

        let cfilename = CString::new(filename).unwrap_or_default();
        let compile_options = CompileOptionsWrapper::new(self.runtime.cx_no_gc(), cfilename, 1);
        let res = evaluate_script(
            self.runtime.cx(),
            global.handle(),
            source,
            rval.handle_mut(),
            compile_options,
        );

        match res {
            Ok(()) => Ok(convert(rval.get())),
            Err(()) => Err(JsError {
                message: format!("uncaught exception while evaluating {filename}"),
            }),
        }
    }

    fn engine_name(&self) -> &'static str {
        "SpiderMonkey (mozjs)"
    }
}

/// Convert a SpiderMonkey `Value` into our simplified [`JsValue`]. Strings and
/// objects are reported as typed placeholders (decoding them needs a rooted
/// conversion, a follow-on).
fn convert(v: mozjs::jsapi::Value) -> JsValue {
    if v.is_undefined() {
        JsValue::Undefined
    } else if v.is_null() {
        JsValue::Null
    } else if v.is_boolean() {
        JsValue::Bool(v.to_boolean())
    } else if v.is_int32() {
        JsValue::Number(v.to_int32() as f64)
    } else if v.is_double() {
        JsValue::Number(v.to_double())
    } else if v.is_string() {
        JsValue::Str("[string]".to_string())
    } else {
        JsValue::Str("[object]".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_real_javascript() {
        let mut rt = SpiderMonkeyRuntime::new().expect("boot SpiderMonkey");
        assert_eq!(rt.engine_name(), "SpiderMonkey (mozjs)");
        assert_eq!(rt.eval("40 + 2", "t.js").unwrap(), JsValue::Number(42.0));
        assert_eq!(rt.eval("2 > 5", "t.js").unwrap(), JsValue::Bool(false));
        assert_eq!(
            rt.eval("let a = 3; a * a", "t.js").unwrap(),
            JsValue::Number(9.0)
        );
    }
}
