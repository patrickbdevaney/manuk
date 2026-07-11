//! G1 — **real-site visual fidelity vs Chromium** (ADR-010, amended).
//!
//! The box-probe parity gate compares `getBoundingClientRect` on 30 *synthetic* pages. That is a
//! rigorous signal but it is **not the user's experience**: a page can pass box tolerance and still
//! look wrong — missing backgrounds, dropped shadows, wrong fonts, an unpainted element. And real
//! modern sites aren't in that corpus at all.
//!
//! So this gate does what a person would do: **render the real page, screenshot Chromium rendering
//! the same page, and compare the pixels.** Both are full renders through the real pipeline
//! (external CSS + images + JS), not a side channel.
//!
//! **Comparison method.** A raw pixel diff is useless here — font hinting and antialiasing differ
//! between any two engines and would swamp the signal. Instead both images are reduced to a coarse
//! **block grid** (mean RGB per cell). That is deliberately blind to glyph-level AA but *very*
//! sensitive to what actually matters: layout displacement, a missing background, an unpainted box,
//! a wrong colour. The score is the fraction of blocks whose mean colour agrees within tolerance.

use std::path::Path;

use anyhow::{Context, Result};

/// Per-page fidelity result.
pub struct Fidelity {
    pub name: String,
    /// Fraction of blocks agreeing with Chromium, 0.0–1.0.
    pub score: f64,
    /// Blocks that differ (for a quick "where is it wrong" read).
    pub differing: usize,
    pub total: usize,
}

/// Grid resolution — coarse enough to ignore glyph AA, fine enough to catch a missing element.
const GRID: u32 = 40;
/// Per-channel mean tolerance for a block to count as "agreeing".
const TOL: f64 = 26.0;

/// Mean RGB of each grid cell of an RGBA8 image.
fn block_means(rgba: &[u8], w: u32, h: u32) -> Vec<[f64; 3]> {
    let mut out = Vec::with_capacity((GRID * GRID) as usize);
    for gy in 0..GRID {
        for gx in 0..GRID {
            let (x0, x1) = (gx * w / GRID, ((gx + 1) * w / GRID).min(w));
            let (y0, y1) = (gy * h / GRID, ((gy + 1) * h / GRID).min(h));
            let (mut r, mut g, mut b, mut n) = (0f64, 0f64, 0f64, 0f64);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = ((y * w + x) * 4) as usize;
                    if i + 2 < rgba.len() {
                        r += rgba[i] as f64;
                        g += rgba[i + 1] as f64;
                        b += rgba[i + 2] as f64;
                        n += 1.0;
                    }
                }
            }
            let n = n.max(1.0);
            out.push([r / n, g / n, b / n]);
        }
    }
    out
}

fn load_rgba(path: &Path) -> Result<(Vec<u8>, u32, u32)> {
    let img = image::open(path).with_context(|| format!("opening {}", path.display()))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok((rgba.into_raw(), w, h))
}

/// Compare two rendered PNGs; returns the fraction of grid blocks that agree.
pub fn compare(manuk: &Path, chrome: &Path, name: &str) -> Result<Fidelity> {
    let (a, aw, ah) = load_rgba(manuk)?;
    let (b, bw, bh) = load_rgba(chrome)?;
    let ma = block_means(&a, aw, ah);
    let mb = block_means(&b, bw, bh);
    let total = ma.len().min(mb.len());
    let mut differing = 0usize;
    for i in 0..total {
        let d = (0..3)
            .map(|c| (ma[i][c] - mb[i][c]).abs())
            .fold(0.0f64, f64::max);
        if d > TOL {
            differing += 1;
        }
    }
    let score = if total == 0 {
        0.0
    } else {
        1.0 - (differing as f64 / total as f64)
    };
    Ok(Fidelity { name: name.to_string(), score, differing, total })
}

/// Write a **side-by-side** composite (Manuk left, Chromium right, a divider between) so the pair
/// can be inspected as ONE image — the eyeball check the numeric score cannot replace.
pub fn write_side_by_side(manuk: &Path, chrome: &Path, dest: &Path) -> Result<()> {
    let (a, aw, ah) = load_rgba(manuk)?;
    let (b, bw, bh) = load_rgba(chrome)?;
    let h = ah.max(bh);
    let gap = 8u32;
    let w = aw + gap + bw;
    let mut out = vec![255u8; (w * h * 4) as usize];
    let mut blit = |src: &[u8], sw: u32, sh: u32, ox: u32| {
        for y in 0..sh {
            for x in 0..sw {
                let si = ((y * sw + x) * 4) as usize;
                let di = ((y * w + x + ox) * 4) as usize;
                if si + 3 < src.len() && di + 3 < out.len() {
                    out[di..di + 4].copy_from_slice(&src[si..si + 4]);
                }
            }
        }
    };
    blit(&a, aw, ah, 0);
    blit(&b, bw, bh, aw + gap);
    // Divider.
    for y in 0..h {
        for x in aw..(aw + gap) {
            let di = ((y * w + x) * 4) as usize;
            if di + 3 < out.len() {
                out[di..di + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
    }
    let img = image::RgbaImage::from_raw(w, h, out).context("composite")?;
    img.save(dest).with_context(|| format!("writing {}", dest.display()))?;
    Ok(())
}

/// Print the fidelity report + the gate verdict against `floor`.
pub fn report(rows: &[Fidelity], floor: f64) -> bool {
    println!("\n=== G1 · REAL-SITE VISUAL FIDELITY vs Chromium ===\n");
    println!("{:<26} {:>9} {:>10} {:>8}", "page", "fidelity", "diff/total", "verdict");
    let mut all_ok = true;
    for r in rows {
        let ok = r.score >= floor;
        if !ok {
            all_ok = false;
        }
        println!(
            "{:<26} {:>8.1}% {:>10} {:>8}",
            r.name,
            r.score * 100.0,
            format!("{}/{}", r.differing, r.total),
            if ok { "ok" } else { "BELOW" }
        );
    }
    let mean = if rows.is_empty() {
        0.0
    } else {
        rows.iter().map(|r| r.score).sum::<f64>() / rows.len() as f64
    };
    println!("\nMEAN FIDELITY: {:.1}%   (floor {:.0}%)", mean * 100.0, floor * 100.0);
    println!("Side-by-side composites written — LOOK at them: a page can score well and still be\n\
              visibly wrong, and the score cannot tell you *why*.\n");
    all_ok
}
