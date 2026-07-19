//! # WebVTT captions — media step M7
//!
//! **The one part of the media stack that needs no decoder**, which is why it is reachable now:
//! a caption file is text, and turning it into timed cues is parsing plus arithmetic. No feature
//! gate, no C, no dependency.
//!
//! Captions are table stakes rather than polish — an accessibility requirement, and how a large
//! fraction of viewers watch video at all.
//!
//! ## `active_at` returns a LIST, and that is the decision this module turns on
//!
//! **Cues overlap.** Two speakers captioned simultaneously, a speaker label held across several
//! lines, a translation over an on-screen sign — all routinely put two cues on screen at once. So
//! "what is showing at time `t`" is a *set*, not an item.
//!
//! Returning `Option<&Cue>` compiles, reads as perfectly reasonable, and silently drops the second
//! speaker for the entire span where both are live. It is the same shape as tick 254's
//! `selectedOptions`: the plural question answered in the singular, where the wrong answer is a
//! valid-looking one rather than an error.
//!
//! ## What a real caption file looks like, and the three things that break naive parsers
//!
//! - **Hours are optional.** `00:01.500 --> 00:04.000` is the common form. A parser that demands
//!   `HH:MM:SS.mmm` rejects most files in the wild — and rejects them *wholesale*, so the video
//!   simply has no captions rather than having slightly wrong ones.
//! - **`NOTE` blocks are comments.** They are shaped like cues and are not cues. Rendering one puts
//!   the translator's private remark on screen over the video.
//! - **Cue settings share the timestamp line.** `--> 00:04.000 align:start position:50%` — the
//!   settings are not part of the caption, and a parser that keeps the rest of the line prints
//!   `align:start position:50%` to the viewer.
//!
//! The fractional separator is `.` (SRT uses `,`) and the arrow is `-->`; both are checked, because
//! `.srt` renamed to `.vtt` is a thing people do and the honest answer is a refusal.

/// One caption: when it is on screen, and what it says.
#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    /// The optional identifier line preceding the timestamp.
    pub id: Option<String>,
    pub start: f64,
    pub end: f64,
    /// The caption text, newlines preserved. Cue settings are NOT part of this.
    pub text: String,
}

impl Cue {
    /// Is this cue on screen at `t`?
    ///
    /// Half-open: `[start, end)`. A cue ending exactly when the next begins must not render both
    /// for one instant — back-to-back cues are the normal case, not an edge case.
    pub fn active_at(&self, t: f64) -> bool {
        t >= self.start && t < self.end
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VttError {
    /// The file does not begin with the `WEBVTT` signature.
    NotWebVtt,
}

impl std::fmt::Display for VttError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VttError::NotWebVtt => write!(f, "not a WebVTT file (missing the WEBVTT signature)"),
        }
    }
}

impl std::error::Error for VttError {}

/// A parsed caption track.
#[derive(Debug, Clone, Default)]
pub struct VttTrack {
    cues: Vec<Cue>,
}

impl VttTrack {
    /// Parse a WebVTT file.
    ///
    /// **A malformed cue is SKIPPED, not fatal.** One unparseable timestamp in a 900-cue file must
    /// not cost the viewer the other 899 — every real player is lenient here, and being stricter
    /// than the ecosystem means captions vanish entirely on files that work everywhere else. The
    /// missing signature IS fatal, because that is the one case where we do not know what we are
    /// reading.
    pub fn parse(src: &str) -> Result<Self, VttError> {
        let src = src.trim_start_matches('\u{feff}');
        let mut lines = src.lines();

        // The signature may carry a trailing header (`WEBVTT - Some Title`).
        match lines.next() {
            Some(first) if first.trim_start().starts_with("WEBVTT") => {}
            _ => return Err(VttError::NotWebVtt),
        }

        let mut cues = Vec::new();
        let rest: Vec<&str> = lines.collect();
        let mut i = 0usize;

        while i < rest.len() {
            let line = rest[i].trim();

            if line.is_empty() {
                i += 1;
                continue;
            }

            // `NOTE` comments run to the next blank line. Shaped like a cue, and not one.
            if line == "NOTE" || line.starts_with("NOTE ") || line.starts_with("NOTE\t") {
                i += 1;
                while i < rest.len() && !rest[i].trim().is_empty() {
                    i += 1;
                }
                continue;
            }

            // `STYLE` and `REGION` blocks are likewise not cues.
            if line == "STYLE" || line == "REGION" {
                i += 1;
                while i < rest.len() && !rest[i].trim().is_empty() {
                    i += 1;
                }
                continue;
            }

            // A cue is [id]\n<timings>\n<payload...>. The id line is optional, so the timing line
            // is either this line or the next.
            let (id, timing_idx) = if line.contains("-->") {
                (None, i)
            } else if i + 1 < rest.len() && rest[i + 1].contains("-->") {
                (Some(line.to_string()), i + 1)
            } else {
                // Neither this line nor the next is a timing line: not a cue we understand.
                i += 1;
                continue;
            };

            let Some((start, end)) = parse_timing(rest[timing_idx]) else {
                i = timing_idx + 1;
                continue;
            };

            let mut payload: Vec<&str> = Vec::new();
            let mut j = timing_idx + 1;
            while j < rest.len() && !rest[j].trim().is_empty() {
                payload.push(rest[j]);
                j += 1;
            }

            cues.push(Cue {
                id,
                start,
                end,
                text: payload.join("\n").trim_end().to_string(),
            });
            i = j;
        }

        // Start order, so `active_at` reads in the order a viewer sees them. A file whose cues are
        // out of order is unusual but legal, and rendering them out of order stacks the captions
        // upside down.
        cues.sort_by(|a, b| {
            a.start
                .partial_cmp(&b.start)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(Self { cues })
    }

    /// **Every cue on screen at `t`** — see the module note on why this is plural.
    pub fn active_at(&self, t: f64) -> Vec<&Cue> {
        self.cues.iter().filter(|c| c.active_at(t)).collect()
    }

    pub fn cues(&self) -> &[Cue] {
        &self.cues
    }

    pub fn len(&self) -> usize {
        self.cues.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cues.is_empty()
    }
}

/// `00:00:01.500 --> 00:00:04.000 align:start position:50%` → `(1.5, 4.0)`.
///
/// Everything after the end timestamp is cue SETTINGS and is discarded here — keeping it would
/// print `align:start position:50%` to the viewer.
fn parse_timing(line: &str) -> Option<(f64, f64)> {
    let (lhs, rhs) = line.split_once("-->")?;
    let start = parse_timestamp(lhs.trim())?;
    // The end timestamp is the FIRST token after the arrow; the rest is settings.
    let end_tok = rhs.trim().split_whitespace().next()?;
    let end = parse_timestamp(end_tok)?;
    Some((start, end))
}

/// `HH:MM:SS.mmm` or `MM:SS.mmm` — **hours are optional**, which is the common form.
fn parse_timestamp(s: &str) -> Option<f64> {
    // The fraction separator is `.` in WebVTT. A `,` is SRT, and saying so beats guessing.
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, f),
        None => (s, ""),
    };
    if whole.contains(',') {
        return None;
    }

    let parts: Vec<&str> = whole.split(':').collect();
    let (h, m, sec) = match parts.as_slice() {
        [h, m, s] => (
            h.parse::<f64>().ok()?,
            m.parse::<f64>().ok()?,
            s.parse::<f64>().ok()?,
        ),
        [m, s] => (0.0, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        _ => return None,
    };
    if !(0.0..60.0).contains(&sec) || m >= 60.0 {
        return None;
    }

    let millis = if frac.is_empty() {
        0.0
    } else {
        // Pad/truncate to milliseconds so `.5` is 500ms, not 5ms.
        let f: String = frac
            .chars()
            .filter(|c| c.is_ascii_digit())
            .take(3)
            .collect();
        if f.is_empty() {
            return None;
        }
        let padded = format!("{f:0<3}");
        padded.parse::<f64>().ok()? / 1000.0
    };

    Some(h * 3600.0 + m * 60.0 + sec + millis)
}
