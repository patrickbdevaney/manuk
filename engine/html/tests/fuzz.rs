//! HTML parser robustness fuzz-lite (P0.6): random markup must never panic in our
//! DOM walk. html5ever itself is fuzz-hardened upstream; this guards our conversion.

use manuk_html::parse;

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

#[test]
fn html_parse_never_panics() {
    let mut s = 0x243F6A8885A308D3u64;
    let alphabet = b"<>/=\" abcdivpsanolh123!-&;#\n\ttable";
    for _ in 0..10_000 {
        let len = (xorshift(&mut s) as usize) % 200;
        let input: String = (0..len)
            .map(|_| alphabet[(xorshift(&mut s) as usize) % alphabet.len()] as char)
            .collect();
        let dom = parse(&input);
        // Exercise the walk paths (descendants + text) — must not panic.
        std::hint::black_box(dom.text_content(dom.root()));
        std::hint::black_box(dom.descendants(dom.root()).count());
    }
}
