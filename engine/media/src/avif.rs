//! # AVIF stills — the blank-hero-image class, riding the `<video>` decoder (tick 355)
//!
//! An AVIF file is an **AV1 keyframe in a HEIF box**. Both halves are borrowed: `avif-parse`
//! (Mozilla-lineage, MPL-2.0 — parse only, it never touches pixels) walks the container to the
//! primary item's OBUs, and the same `re_rav1d` instance that plays `<video>` decodes them. This
//! module is only the join.
//!
//! ## Why this lives here and not in `manuk-page`'s image path
//!
//! The obvious place to decode an image format is next to `image::load_from_memory` — and that
//! is exactly where this must NOT go: `manuk-page` is linked by every gate binary, and the
//! decoder-isolation rule (the reason `audio`/`video`/`av1` are opt-in features) forbids rav1d
//! riding into all of them. So the page's fetcher hands back the bytes it cannot decode, and the
//! SHELL — the one lane that already pays for the decoder — calls this.
//!
//! ## Honest limits, named
//!
//! - **Alpha is not composited yet.** AVIF alpha is a *separate* auxiliary AV1 image
//!   (`alpha_item`); v1 decodes the color item and renders opaque. A transparent hero on a dark
//!   page will show its own background — degraded, visible, better than blank. Follow-on.
//! - **8-bit only**, same as `<video>`: the build is `bitdepth_8`, and a 10/12-bit AVIF returns
//!   an error the caller treats as "not decodable" — never a panic, never a wrong picture.

use crate::av1;
use crate::video::{Frame, VideoError};
use re_rav1d::dav1d;

/// Is this an AVIF file at all? The ISO-BMFF `ftyp` box with an `avif`/`avis` brand, which is
/// how a fetcher routes bytes here without attempting a full parse on every JPEG.
pub fn sniff_avif(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[4..8] == b"ftyp" && matches!(&bytes[8..12], b"avif" | b"avis")
}

/// Decode an AVIF file's primary image to RGBA.
///
/// Errors are the caller's "no" — a malformed container, a 10-bit stream, a truncated OBU all
/// come back as `Err`, and the image simply stays un-rendered exactly as an undecodable JPEG
/// would. Nothing here panics on hostile input: `avif-parse` is fallible by construction and
/// the decoder path is the same one `<video>` already trusts with network bytes.
pub fn decode_avif(bytes: &[u8]) -> Result<Frame, VideoError> {
    let data = avif_parse::read_avif(&mut &bytes[..])
        .map_err(|e| VideoError::Failed(format!("avif container: {e}")))?;
    let mut frame = decode_item(&data.primary_item)?;
    // The alpha auxiliary image (tick 368): a MONOCHROME AV1 picture whose Y plane IS the mask.
    // Decoded through the same path; composited as one write per pixel into the A channel the
    // color conversion already allocates as 255. A failed or mismatched alpha falls back to
    // opaque — degraded and honest, never a crash on hostile input.
    if let Some(alpha_obus) = data.alpha_item.as_deref() {
        if let Ok(alpha) = decode_item_alpha(alpha_obus) {
            if alpha.0 == frame.width as usize && alpha.1 == frame.height as usize {
                for (px, &a) in frame.rgba.chunks_exact_mut(4).zip(alpha.2.iter()) {
                    px[3] = a;
                    // Premultiplied color must be un-premultiplied: the paint path's src-over
                    // expects STRAIGHT alpha, and double-multiplying darkens every edge.
                    if data.premultiplied_alpha && a != 0 && a != 255 {
                        for c in px.iter_mut().take(3) {
                            *c = ((*c as u32 * 255) / a as u32).min(255) as u8;
                        }
                    }
                }
            }
        }
    }
    Ok(frame)
}

/// Decode one item's OBUs to a full [`Frame`] (the color path).
fn decode_item(obus: &[u8]) -> Result<Frame, VideoError> {
    let mut dec = av1::new_decoder()?;
    dec.send_data(obus.to_vec(), None, None, None)
        .map_err(|e| VideoError::Failed(format!("avif item rejected: {e}")))?;
    match dec.get_picture() {
        Ok(pic) => av1::frame_from(&pic),
        Err(dav1d::Error::Again) => Err(VideoError::Failed("avif item produced no picture".into())),
        Err(e) => Err(VideoError::Failed(format!("avif decode: {e}"))),
    }
}

/// Decode the alpha item to `(width, height, mask)` — the picture's Y plane, one byte per pixel.
///
/// Deliberately NOT `frame_from`: the alpha image is monochrome and its Y values are the mask
/// itself — running them through a YUV color matrix would rescale the very numbers we need.
fn decode_item_alpha(obus: &[u8]) -> Result<(usize, usize, Vec<u8>), VideoError> {
    let mut dec = av1::new_decoder()?;
    dec.send_data(obus.to_vec(), None, None, None)
        .map_err(|e| VideoError::Failed(format!("alpha item rejected: {e}")))?;
    let pic = match dec.get_picture() {
        Ok(p) => p,
        Err(e) => return Err(VideoError::Failed(format!("alpha decode: {e}"))),
    };
    if pic.bit_depth() != 8 {
        return Err(VideoError::Unsupported(format!(
            "{}-bit alpha (8-bit build)",
            pic.bit_depth()
        )));
    }
    let (w, h) = (pic.width() as usize, pic.height() as usize);
    let y = pic.plane(dav1d::PlanarImageComponent::Y);
    let stride = pic.stride(dav1d::PlanarImageComponent::Y) as usize;
    let mut mask = Vec::with_capacity(w * h);
    for row in 0..h {
        mask.extend_from_slice(&y[row * stride..row * stride + w]);
    }
    Ok((w, h, mask))
}
