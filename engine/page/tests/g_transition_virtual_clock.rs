//! **G_TRANSITION_VIRTUAL_CLOCK — the transition clock never runs behind the JS timer clock.**
//! The event loop jumps its virtual clock to the next due timer without sleeping, so a
//! `setTimeout(...,1000)` read of a 2 s transition saw ~1% progress on wall time alone.
//! RED, run: make `set_virtual_ms` a no-op -> `realtime_now_ms()` stays near 0.
#[test]
fn realtime_now_follows_the_virtual_timer_clock() {
    manuk_css::transition::set_virtual_ms(1000.0);
    assert!(manuk_css::transition::realtime_now_ms() >= 1000.0);
    manuk_css::transition::set_virtual_ms(10.0); // monotonic: never rewinds
    assert!(manuk_css::transition::realtime_now_ms() >= 1000.0);
}
