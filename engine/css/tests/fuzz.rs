//! Parser robustness fuzz-lite (P0.6): random inputs must never panic.
//!
//! Deterministic (fixed-seed xorshift) so it is reproducible in CI on stable Rust.
//! This targets the from-scratch CSS parser/values only — **never** SpiderMonkey's
//! JIT/GC (that stays vendored per CLAUDE.md's modification boundary).

use manuk_css::values::{parse_color, parse_dim};
use manuk_css::Stylesheet;

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn random_string(state: &mut u64, alphabet: &[u8], max_len: usize) -> String {
    let len = (xorshift(state) as usize) % max_len;
    (0..len)
        .map(|_| alphabet[(xorshift(state) as usize) % alphabet.len()] as char)
        .collect()
}

#[test]
fn stylesheet_parse_never_panics() {
    let mut s = 0x9E3779B97F4A7C15u64;
    let alphabet = b"{}();:.#*abcdefg0123 \n\t/!important@media,>+~[]\"'%pxem-";
    for _ in 0..30_000 {
        let input = random_string(&mut s, alphabet, 160);
        std::hint::black_box(Stylesheet::parse(&input));
    }
}

#[test]
fn value_parsers_never_panic() {
    let mut s = 0xD1B54A32D192ED03u64;
    let alphabet = b"0123456789.-+%pxemrgba()#, autrl";
    for _ in 0..30_000 {
        let v = random_string(&mut s, alphabet, 40);
        std::hint::black_box(parse_color(&v));
        std::hint::black_box(parse_dim(&v, 16.0));
    }
}
