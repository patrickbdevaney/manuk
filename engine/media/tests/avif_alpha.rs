//! # avif_alpha — the transparent hero's alpha survives to the A channel
//!
//! The t355 residue closed (tick 368): the alpha auxiliary image decodes through the same dav1d
//! path and lands in the A channel as STRAIGHT alpha. The claims are on CONTENT: the mask fixture
//! must yield a VARYING A channel with genuinely transparent pixels, and the opaque red fixture
//! must stay all-255 — an alpha path that fires on alphaless files would fade every JPEG-class
//! image on the web.

use manuk_media::decode_avif;

const MASKED: &[u8] = include_bytes!("data/red-with-alpha-8bpc.avif");
const OPAQUE: &[u8] = include_bytes!("data/red-full-range-420-8bpc.avif");

#[test]
fn avif_alpha() {
    let f = decode_avif(MASKED).expect("the alpha-mask fixture must decode");
    let alphas: Vec<u8> = f.rgba.chunks_exact(4).map(|px| px[3]).collect();
    assert!(
        alphas.iter().any(|&a| a < 250),
        "the alpha item must produce NON-OPAQUE pixels — an ignored alpha item renders every \
         transparent hero opaque (min alpha seen: {})",
        alphas.iter().min().unwrap()
    );
    assert!(
        alphas.iter().any(|&a| a > 200),
        "the image must also keep substantially-opaque pixels — an all-transparent result is a \
         scrambled read"
    );

    let o = decode_avif(OPAQUE).expect("the opaque fixture still decodes");
    assert!(
        o.rgba.chunks_exact(4).all(|px| px[3] == 255),
        "an AVIF with NO alpha item must stay fully opaque — an alpha path that fires on \
         alphaless files fades images"
    );
    let center = ((o.height / 2) * o.width + o.width / 2) as usize * 4;
    let px = &o.rgba[center..center + 4];
    assert!(
        px[0] > 200 && px[1] < 80,
        "compositing must not disturb the color plane — red stays red, got {px:?}"
    );
}
