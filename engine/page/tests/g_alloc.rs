//! **G_ALLOC** — the allocation-rate gate.
//!
//! ## Why this gate exists
//!
//! Every performance floor we had (F1–F3) measured *loading*: parse, cascade, layout, paint, on an
//! idle queue. All of them were green while the browser was unusable, because the regression was
//! not in loading — it was in the **marginal cost of one input event**. Publishing the layout and
//! style snapshots into the JS world *cloned* them: an 18,630-entry rect map and 18,630
//! `ComputedStyle` structs, each with heap-allocated font lists. Per wheel event. A trackpad
//! delivers dozens per frame.
//!
//! A load-time bench cannot see that, ever. It is not a gap you patch once — it is a **category of
//! gate** that was missing.
//!
//! ## What it asserts
//!
//! 1. **Near-zero allocation when nothing is listening.** The overwhelming majority of pages
//!    register no `scroll` listener and no observer. For those, a scroll must cost essentially
//!    nothing — not "a bit less than before".
//! 2. **Sub-linear in DOM size when something *is* listening.** The work of telling a page it
//!    scrolled must not scale with the size of the document. That is the specific shape of the bug:
//!    O(n) per event, sixty times a second.
//!
//! Both are asserted against a **large** DOM, because the bug was invisible on a small one.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// A counting allocator. Only counts while `ARMED` — so the page's own construction (which
/// legitimately allocates a great deal) does not drown the signal from one event.
struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
static ARMED: AtomicBool = AtomicBool::new(false);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ARMED.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(l.size(), Ordering::Relaxed);
        }
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        if ARMED.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(new.saturating_sub(l.size()), Ordering::Relaxed);
        }
        System.realloc(p, l, new)
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Run `f` with the allocator counting, and report `(allocations, bytes)`.
fn measure(f: impl FnOnce()) -> (usize, usize) {
    ALLOCS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    ARMED.store(true, Ordering::Relaxed);
    f();
    ARMED.store(false, Ordering::Relaxed);
    (
        ALLOCS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    )
}

/// A document with `n` elements — big enough that O(n)-per-event work is unmistakable.
fn big_page(n: usize, with_listener: bool) -> String {
    let mut body = String::with_capacity(n * 40);
    for i in 0..n {
        body.push_str(&format!(
            "<div id=\"d{i}\" class=\"row c{}\"><span>item {i}</span></div>",
            i % 7
        ));
    }
    let script = if with_listener {
        "<script>window.addEventListener('scroll', function(){ window.__n = (window.__n||0)+1; });</script>"
    } else {
        "<script>1;</script>"
    };
    format!("<!doctype html><html><body>{body}{script}</body></html>")
}

/// **The gate.** A scroll on a page that is not listening must cost almost nothing, and a scroll on
/// a page that *is* listening must not scale with the document.
#[test]
#[ignore = "needs the shipping (SpiderMonkey) configuration: cargo test -p manuk-page --features spidermonkey --test g_alloc -- --ignored"]
fn a_scroll_does_not_allocate_per_dom_node() {
    let fonts = manuk_text::FontContext::new();

    // --- 1. Nobody is listening. This is the common case, and it must be free. ---
    let mut quiet =
        manuk_page::Page::load(&big_page(2_000, false), "https://g.test/", &fonts, 1200.0);
    quiet.view_changed(0.0, 1200.0, 800.0, true); // warm any one-time setup
    let (quiet_allocs, quiet_bytes) = measure(|| {
        for i in 0..10 {
            quiet.view_changed(i as f32 * 10.0, 1200.0, 800.0, true);
        }
    });
    assert!(
        quiet_allocs < 200,
        "G_ALLOC: ten scrolls on a 2,000-element page with NO scroll listener and NO observer \
         allocated {quiet_allocs} times ({quiet_bytes} bytes). A page that is not listening must \
         cost essentially nothing to notify — this is the shape of the regression that made \
         scrolling unusable while every other gate stayed green."
    );

    // --- 2. Someone IS listening. The cost must not scale with the DOM. ---
    let mut small = manuk_page::Page::load(&big_page(500, true), "https://g.test/", &fonts, 1200.0);
    let mut large =
        manuk_page::Page::load(&big_page(4_000, true), "https://g.test/", &fonts, 1200.0);
    small.view_changed(0.0, 1200.0, 800.0, true);
    large.view_changed(0.0, 1200.0, 800.0, true);

    let (small_allocs, _) = measure(|| {
        for i in 0..5 {
            small.view_changed(i as f32 * 10.0, 1200.0, 800.0, true);
        }
    });
    let (large_allocs, _) = measure(|| {
        for i in 0..5 {
            large.view_changed(i as f32 * 10.0, 1200.0, 800.0, true);
        }
    });

    // 8× the DOM. Linear-per-node work would show up as roughly 8× the allocations. Allow a
    // generous 3× — we are catching an ORDER-of-magnitude shape, not micro-tuning.
    let ratio = large_allocs as f64 / small_allocs.max(1) as f64;
    assert!(
        ratio < 3.0,
        "G_ALLOC: notifying a listening page of a scroll scales with DOM SIZE \
         (500 elements → {small_allocs} allocs; 4,000 elements → {large_allocs} allocs; {ratio:.1}×). \
         The work of telling a page it scrolled must not be O(nodes) — that is exactly the \
         clone-the-whole-style-map regression, sixty times a second, on the UI thread."
    );

    // Tear SpiderMonkey down in order, as G_TEARDOWN requires of every process that starts it.
    // A test that passes and then aborts on the way out has not passed.
    drop(quiet);
    drop(small);
    drop(large);
    manuk_js::shutdown();
}
