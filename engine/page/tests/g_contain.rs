//! **G_CONTAIN — Bar 0: a panic kills the PAGE, not the process (METHODOLOGY Part 23.2/23.3).**
//!
//! Bar 1 asks "does this look broken". Bar 0 asks something more fundamental that has to be true
//! first: *does the engine ever take the whole browser down.* A page that renders perfectly nine
//! times and kills the process the tenth violates Bar 0 no matter how good the nine looked.
//!
//! apple.com core-dumped this browser. The specific cause is fixed — but **you will not prevent every
//! crash-class bug before Bar 1**, and that is not pessimism, it is the premise of the whole
//! pattern-coverage strategy: the tail of patterns we do not cover is where the panics live, and the
//! tail is infinite. So the requirement is containment, not prevention. The failure mode for an
//! uncovered pattern must be "this tab shows an error and the browser carries on", never "everything
//! the user had open is gone".
//!
//! This test deliberately panics inside a page build and asserts the process survives and keeps
//! working. It is only possible at all because the release profile no longer sets `panic = "abort"` —
//! with abort, `catch_unwind` cannot exist and Bar 0 is unreachable by construction.

/// The renderer falls over. The browser does not.
#[test]
fn a_panic_during_a_page_build_does_not_kill_the_process() {
    // Silence the panic hook's backtrace spam: the panic here is the point, not a failure.
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let out = manuk_page::contained("deliberate", || {
        panic!("the renderer fell over on some pattern we do not cover yet");
    });
    std::panic::set_hook(prev);

    assert!(
        out.is_none(),
        "a panicking build must report failure, not a half-built page"
    );

    // And — the whole point — we are still running. Everything below this line executing at all is
    // the assertion. Under `panic = "abort"` the process would be gone.
    let still_alive: u32 = (1..=10).sum();
    assert_eq!(
        still_alive, 55,
        "the process is alive and doing arithmetic after a contained panic"
    );
}

/// Containment must not swallow SUCCESS, which is the obvious way to write this wrong: a `contained`
/// that always returned `None` would pass the test above and break the browser completely.
#[test]
fn containment_does_not_swallow_the_normal_path() {
    let out = manuk_page::contained("ok", || 42);
    assert_eq!(
        out,
        Some(42),
        "a build that does NOT panic must come back intact — a containment boundary that eats the \
         happy path is not containment, it is a browser that never renders anything"
    );
}

/// The failure must remain VISIBLE (Part 22.1: no silent failure). A contained panic is still a bug;
/// making it survivable must not make it invisible, or the fix that keeps the browser alive is also
/// the fix that hides why it nearly died.
#[test]
fn a_contained_panic_is_still_reported() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read page/src/lib.rs");
    let f = src
        .split("pub fn contained")
        .nth(1)
        .expect("`contained` exists");
    let body = &f[..f.find("\n}").unwrap_or(f.len())];
    assert!(
        body.contains("tracing::error!"),
        "a contained panic MUST be logged at error level and reach the discovery pipeline. \
         Absorbing it silently would make the browser survive and make the bug invisible — which is \
         how a crash becomes a permanent, unexplained 'this site just doesn't work'."
    );
}
