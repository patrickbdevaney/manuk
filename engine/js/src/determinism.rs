//! `--deterministic` (R3.1): the same page, run twice, observes the same clock and the same dice.
//!
//! What a page can see that differs between two runs of the same bytes: wall time (`Date`,
//! `performance.now`), entropy (`Math.random`, `crypto`), and how much work fitted inside a
//! wall-clock budget (a loaded machine cuts a drain or a load short where an idle one does not).
//! Timers were already on a virtual clock (`__now`, see the event-loop prelude). This module pins the
//! other three:
//!
//! * `Date` / `performance.now` read the virtual clock plus a per-read tick, so a busy-wait on the
//!   clock terminates, and does so after the same number of reads every run;
//! * `Math.random` and `crypto.*` draw from a stream seeded by `MANUK_SEED`;
//! * the drain and load budgets become generous *backstops* ([`DRAIN_BACKSTOP_MS`],
//!   [`LOAD_BACKSTOP_MS`]) instead of the tuned interactive ones, so load on the machine cannot move
//!   where a page is cut. Logical bounds (`MAX_TASKS_PER_DRAIN`) are unchanged and do the real work.
//!
//! Not covered here (named so nobody assumes it): network record/replay, fdlibm/TZ/locale.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub const DEFAULT_SEED: u64 = 0x4d41_4e55_4b00_0001;
/// 2026-01-01T00:00:00Z. Fixed so a stored run is not date-dependent.
pub const EPOCH_MS: u64 = 1_767_225_600_000;
pub const DRAIN_BACKSTOP_MS: u128 = 120_000;
pub const LOAD_BACKSTOP_MS: u64 = 300_000;

static FORCED: AtomicBool = AtomicBool::new(false);
static CRYPTO_STATE: AtomicU64 = AtomicU64::new(DEFAULT_SEED);

/// On when `MANUK_DETERMINISTIC` is set to anything but `""`/`0`. An env var rather than a flag
/// parameter so every binary (`manuk-mcp`, `a11y-dump`, the GUI) and every child process gets it.
pub fn enabled() -> bool {
    FORCED.load(Ordering::Relaxed)
        || std::env::var("MANUK_DETERMINISTIC")
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false)
}

/// The `--deterministic` flag of a binary: no env mutation, so it is safe after threads exist.
pub fn enable() {
    FORCED.store(true, Ordering::Relaxed);
}

pub fn seed() -> u64 {
    std::env::var("MANUK_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_SEED)
}

/// Called per page install: each page starts from the same stream position.
pub fn reset() {
    CRYPTO_STATE.store(seed(), Ordering::SeqCst);
}

/// splitmix64 over an atomic counter. Not a CSPRNG, and it must not be one: this mode trades
/// unpredictability for repeatability, and says so (`crypto` is only reached when it is on).
pub fn next_u64() -> u64 {
    let mut z = CRYPTO_STATE
        .fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::SeqCst)
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn fill(buf: &mut [u8]) {
    for chunk in buf.chunks_mut(8) {
        let b = next_u64().to_le_bytes();
        chunk.copy_from_slice(&b[..chunk.len()]);
    }
}

/// JS half. Evaluated BEFORE the event-loop prelude so the prelude's `performance` origin
/// (`t0 = Date.now()`) is taken on the logical clock too. `__now` is read lazily.
pub fn prelude_js() -> String {
    format!(
        r#"(function () {{
  var D = Date, EPOCH = {epoch}, reads = 0;
  var logical = function () {{ return (globalThis.__now || 0) + (++reads) * 0.001; }};
  var F = function Date(a, b, c, d, e, f, g) {{
    if (!new.target) {{ return new D(EPOCH + Math.floor(logical())).toString(); }}
    if (arguments.length === 0) {{ return Reflect.construct(D, [EPOCH + Math.floor(logical())], new.target); }}
    return Reflect.construct(D, arguments, new.target);
  }};
  F.prototype = D.prototype;
  try {{ Object.defineProperty(D.prototype, 'constructor', {{ value: F, writable: true, configurable: true }}); }} catch (e) {{}}
  F.now = function now() {{ return EPOCH + Math.floor(logical()); }};
  F.parse = D.parse; F.UTC = D.UTC;
  globalThis.Date = F;
  var s0 = {s0}, s1 = {s1}, s2 = {s2}, s3 = {s3};
  Math.random = function random() {{
    var t = s1 << 9, r = s0 * 5; r = ((r << 7) | (r >>> 25)) * 9;
    s2 ^= s0; s3 ^= s1; s1 ^= s2; s0 ^= s3; s2 ^= t; s3 = (s3 << 11) | (s3 >>> 21);
    return ((r >>> 0) + ((s1 >>> 0) & 0x1fffff) * 4294967296) / 9007199254740992;
  }};
}})();"#,
        epoch = EPOCH_MS,
        s0 = (seed() & 0xffff_ffff) as u32 | 1,
        s1 = (seed() >> 32) as u32 ^ 0x9e37_79b9,
        s2 = 0x85eb_ca6b_u32,
        s3 = 0xc2b2_ae35_u32,
    )
}
