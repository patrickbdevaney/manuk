//! On-disk SpiderMonkey stencil cache for page scripts (W4.E3.a).
//!
//! A page `<script>` is compiled to a stencil, the stencil is XDR-encoded and stored under
//! `$MANUK_HTTP_CACHE_DIR/stencil/<key>`; the next visit decodes + instantiates it instead of
//! parsing the source. The key is `sha256(build id, source name, source bytes)`, so a changed
//! script body, a different engine build, or a different source name is a different file — a
//! stale stencil can never be picked up. SpiderMonkey also checks the build id inside the XDR
//! header, so a mutated key still fails closed (decode error → recompile).
//!
//! `EncodeStencil` is blacklisted in mozjs's generated bindings (its `TranscodeBuffer` is a
//! `mozilla::Vector`), so it is declared here by its mangled name over a layout-compatible
//! `{begin, len, cap}` struct (`MallocAllocPolicy`, no inline storage, release build).

use std::ffi::CString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use mozjs::jsapi::{DelazificationOption, InstantiateOptions, ReadOnlyDecodeOptions};
use mozjs::jsval::UndefinedValue;
use mozjs::realm::AutoRealm;
use mozjs::rooted;
use mozjs::rust::{
    transform_str_to_source_text, wrappers2, CompileOptionsWrapper, HandleObject, Runtime,
};
use sha2::{Digest, Sha256};

static HITS: AtomicU64 = AtomicU64::new(0);
static MISSES: AtomicU64 = AtomicU64::new(0);

/// `(hits, misses)` since process start: a hit decoded a stored stencil, a miss compiled one.
pub fn stats() -> (u64, u64) {
    (HITS.load(Ordering::Relaxed), MISSES.load(Ordering::Relaxed))
}

#[repr(C)]
struct RawVec {
    begin: *mut u8,
    len: usize,
    cap: usize,
}

impl RawVec {
    fn empty() -> Self {
        // mozilla::Vector with zero inline capacity starts at the sentinel `sizeof(T)`.
        RawVec {
            begin: 1 as *mut u8,
            len: 0,
            cap: 0,
        }
    }
}

extern "C" {
    fn malloc(n: usize) -> *mut u8;
    fn free(p: *mut u8);
    #[link_name = "_ZN2JS13EncodeStencilEP9JSContextPN2js8frontend32InitialStencilAndDelazificationsERN7mozilla6VectorIhLm0ENS6_17MallocAllocPolicyEEE"]
    fn encode_stencil(
        cx: *mut mozjs::jsapi::JSContext,
        st: *mut std::ffi::c_void,
        buf: *mut RawVec,
    ) -> u8;
}

/// The build id: changes whenever the running binary does.
fn build_id() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let meta = std::env::current_exe().and_then(std::fs::metadata);
        let (len, mtime) = meta
            .map(|m| {
                let t = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                (m.len(), t)
            })
            .unwrap_or((0, 0));
        format!("manuk-sm140-{len}-{mtime}")
    })
}

/// Supplies SpiderMonkey's process build id (the embedder must; XDR refuses to run without it).
unsafe extern "C" fn build_id_op(out: *mut RawVec) -> bool {
    let id = build_id().as_bytes();
    let p = malloc(id.len().max(1));
    if p.is_null() {
        return false;
    }
    std::ptr::copy_nonoverlapping(id.as_ptr(), p, id.len());
    *out = RawVec {
        begin: p,
        len: id.len(),
        cap: id.len(),
    };
    true
}

fn ensure_build_id_op() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        mozjs::jsapi::SetProcessBuildIdOp(Some(std::mem::transmute::<
            unsafe extern "C" fn(*mut RawVec) -> bool,
            unsafe extern "C" fn(*mut mozjs::jsapi::JS::BuildIdCharVector) -> bool,
        >(build_id_op)));
    });
}

pub fn cache_dir() -> Option<PathBuf> {
    if std::env::var_os("MANUK_NO_STENCIL").is_some() {
        return None;
    }
    std::env::var_os("MANUK_HTTP_CACHE_DIR")
        .filter(|d| !d.is_empty())
        .map(|d| PathBuf::from(d).join("stencil"))
}

/// The cache key for one script under one engine build.
pub fn key(build_id: &str, name: &str, src: &str) -> String {
    let mut h = Sha256::new();
    for part in [build_id.as_bytes(), name.as_bytes(), src.as_bytes()] {
        h.update((part.len() as u64).to_le_bytes());
        h.update(part);
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn instantiate_and_run(
    rt: &mut Runtime,
    global: HandleObject,
    st: *mut mozjs::jsapi::JS::Stencil,
) -> Result<(), ()> {
    let mut realm = AutoRealm::new_from_handle(rt.cx(), global);
    let opts = InstantiateOptions {
        skipFilenameValidation: false,
        hideScriptFromDebugger: false,
        deferDebugMetadata: false,
        eagerDelazificationStrategy_: DelazificationOption::OnDemandOnly,
    };
    let script =
        unsafe { wrappers2::InstantiateGlobalStencil(&realm, &opts, st, std::ptr::null_mut()) };
    if script.is_null() {
        return Err(());
    }
    rooted!(&in(realm) let script = script);
    rooted!(&in(realm) let mut rval = UndefinedValue());
    if unsafe { wrappers2::JS_ExecuteScript(&mut realm, script.handle(), rval.handle_mut()) } {
        Ok(())
    } else {
        Err(())
    }
}

fn decode(rt: &mut Runtime, bytes: &[u8]) -> Option<mozjs::rust::Stencil> {
    let opts = ReadOnlyDecodeOptions {
        borrowBuffer: false,
        usePinnedBytecode: false,
        introducerFilename_: unsafe { std::mem::zeroed() },
        introductionType: std::ptr::null(),
        introductionLineno: 0,
        introductionOffset: 0,
    };
    let range = [bytes.as_ptr(), unsafe { bytes.as_ptr().add(bytes.len()) }];
    let mut out: *mut mozjs::jsapi::JS::Stencil = std::ptr::null_mut();
    let r =
        unsafe { wrappers2::DecodeStencil(rt.cx(), &opts, range.as_ptr() as *const _, &mut out) };
    if r != mozjs::jsapi::JS::TranscodeResult::Ok || out.is_null() {
        unsafe { mozjs::jsapi::JS_ClearPendingException(rt.cx().raw_cx()) };
        return None;
    }
    Some(unsafe {
        std::mem::transmute::<*mut mozjs::jsapi::JS::Stencil, mozjs::rust::Stencil>(out)
    })
}

fn compile(rt: &mut Runtime, src: &str, name: &CString) -> Option<mozjs::rust::Stencil> {
    let opts = CompileOptionsWrapper::new(rt.cx_no_gc(), name.clone(), 1);
    let mut text = transform_str_to_source_text(src);
    let st = unsafe { wrappers2::CompileGlobalScriptToStencil(rt.cx(), opts.ptr, &mut text) };
    let st: mozjs::rust::Stencil = unsafe { std::mem::transmute(st) };
    if st.is_null() {
        // Syntax error: leave it to the plain path to report it.
        unsafe { mozjs::jsapi::JS_ClearPendingException(rt.cx().raw_cx()) };
        return None;
    }
    Some(st)
}

fn store(rt: &mut Runtime, st: &mozjs::rust::Stencil, path: &std::path::Path) {
    let mut buf = RawVec::empty();
    let ok = unsafe { encode_stencil(rt.cx().raw_cx(), **st as *mut _, &mut buf) };
    if ok == 0 && buf.len > 0 {
        let bytes = unsafe { std::slice::from_raw_parts(buf.begin, buf.len) };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension(format!("tmp{}", std::process::id()));
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    } else {
        unsafe { mozjs::jsapi::JS_ClearPendingException(rt.cx().raw_cx()) };
    }
    if buf.cap != 0 {
        unsafe { free(buf.begin) };
    }
}

/// Run a page script (compile + run), through the stencil cache when `$MANUK_HTTP_CACHE_DIR` is
/// set and plain `evaluate_script` otherwise. With `$MANUK_SCRIPT_TIMING=1` it logs one stderr line
/// per script: `manuk-script-ms <hit|miss|plain> <ms> <name>`.
pub fn evaluate_page_script(
    rt: &mut Runtime,
    global: HandleObject,
    src: &str,
    name: CString,
) -> Result<(), ()> {
    let t0 = std::time::Instant::now();
    let h0 = HITS.load(Ordering::Relaxed);
    let r = match evaluate_cached(rt, global, src, &name) {
        Some(r) => r,
        None => {
            rooted!(&in(rt.cx()) let mut rval = UndefinedValue());
            let opts = CompileOptionsWrapper::new(rt.cx_no_gc(), name.clone(), 1);
            mozjs::rust::evaluate_script(rt.cx(), global, src, rval.handle_mut(), opts)
        }
    };
    if std::env::var_os("MANUK_SCRIPT_TIMING").is_some() {
        let kind = if cache_dir().is_none() {
            "plain"
        } else if HITS.load(Ordering::Relaxed) > h0 {
            "hit"
        } else {
            "miss"
        };
        eprintln!(
            "manuk-script-ms {kind} {:.3} {}",
            t0.elapsed().as_secs_f64() * 1000.0,
            name.to_string_lossy()
        );
    }
    r
}

fn evaluate_cached(
    rt: &mut Runtime,
    global: HandleObject,
    src: &str,
    name: &CString,
) -> Option<Result<(), ()>> {
    let dir = cache_dir()?;
    ensure_build_id_op();
    let path = dir.join(key(build_id(), &name.to_string_lossy(), src));
    if let Ok(bytes) = std::fs::read(&path) {
        if let Some(st) = decode(rt, &bytes) {
            HITS.fetch_add(1, Ordering::Relaxed);
            return Some(instantiate_and_run(rt, global, *st));
        }
        let _ = std::fs::remove_file(&path);
    }
    let st = compile(rt, src, name)?;
    MISSES.fetch_add(1, Ordering::Relaxed);
    store(rt, &st, &path);
    Some(instantiate_and_run(rt, global, *st))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_depends_on_build_id_name_and_body() {
        let k = key("b1", "n", "var a=1");
        assert_ne!(k, key("b2", "n", "var a=1"), "build id must be in the key");
        assert_ne!(
            k,
            key("b1", "m", "var a=1"),
            "source name must be in the key"
        );
        assert_ne!(k, key("b1", "n", "var a=2"), "a changed body must miss");
        assert_eq!(k, key("b1", "n", "var a=1"));
        // length-prefixed: moving bytes across the boundary is a different key
        assert_ne!(key("ab", "c", "d"), key("a", "bc", "d"));
    }
}
