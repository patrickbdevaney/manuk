//! **G5 — interaction parity** (ADR-012).
//!
//! A page that *renders* like Chromium but does not *respond* like Chromium is not a browser, it is
//! a screenshot. G1 measures the first frame; this measures what happens after the user touches it.
//!
//! The method is the only honest one available: run the **same scripted interaction** in Manuk and
//! in Chromium, then compare the two resulting documents. Not the two implementations — the two
//! *outcomes*. A click that opens a menu in Chromium and does nothing in Manuk shows up as a
//! difference in the boxes afterwards, no matter how the menu is built.
//!
//! **An interaction that works in Chromium and not in Manuk is a CRITICAL** (ADR-007). Rendering
//! parity that silently sheds interactivity is the failure mode this gate exists to catch.
//!
//! Steps are deliberately few and coarse — `click`, `type`, `scroll` — because these are the ones
//! every site is made of. Exotic gestures can wait; a browser that cannot click cannot be used.

use std::path::Path;

use anyhow::{bail, Context, Result};

/// One step of a scripted interaction. Parsed from a compact textual form so a whole scenario fits
/// on one line of a corpus file.
#[derive(Debug, Clone)]
pub enum Step {
    /// `click:<css>` — click the first match.
    Click(String),
    /// `type:<css>=<text>` — focus the field, set its value, fire `input` + `change`.
    Type(String, String),
    /// `scroll:<px>` — scroll the viewport to an absolute offset.
    Scroll(f32),
}

impl Step {
    pub fn parse(s: &str) -> Result<Step> {
        let s = s.trim();
        if let Some(sel) = s.strip_prefix("click:") {
            return Ok(Step::Click(sel.trim().to_string()));
        }
        if let Some(rest) = s.strip_prefix("type:") {
            let (sel, text) = rest
                .split_once('=')
                .context("type: needs <css>=<text>")?;
            return Ok(Step::Type(sel.trim().to_string(), text.to_string()));
        }
        if let Some(px) = s.strip_prefix("scroll:") {
            return Ok(Step::Scroll(px.trim().parse().context("scroll: needs a number")?));
        }
        bail!("unknown step {s:?} (want click:/type:/scroll:)")
    }

    /// The JavaScript that performs this step **in Chromium**. It must be the same *user-visible*
    /// action, not the same internal call: a real click dispatches a real event, and setting a
    /// field's value without firing `input`/`change` leaves a framework's model of the form
    /// untouched — which is exactly the bug this gate is meant to detect, not to reproduce.
    pub fn to_js(&self) -> String {
        match self {
            Step::Click(sel) => format!(
                "(function(){{var e=document.querySelector({});if(e)e.click();}})();",
                json_str(sel)
            ),
            Step::Type(sel, text) => format!(
                "(function(){{var e=document.querySelector({});if(!e)return;e.focus();\
                 e.value={};e.dispatchEvent(new Event('input',{{bubbles:true}}));\
                 e.dispatchEvent(new Event('change',{{bubbles:true}}));}})();",
                json_str(sel),
                json_str(text)
            ),
            Step::Scroll(y) => format!("window.scrollTo(0,{y});"),
        }
    }
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// One scenario: a page, and what to do to it.
#[derive(Debug, Clone)]
pub struct Scenario {
    pub name: String,
    pub url: String,
    pub steps: Vec<Step>,
}

/// The outcome of running one scenario in both engines.
pub struct InteractionResult {
    pub name: String,
    /// Elements Chromium renders after the interaction that Manuk does not render at all.
    pub missing_after: usize,
    /// Elements both render, but whose box Manuk places differently (beyond tolerance).
    pub moved_after: usize,
    pub probed_after: usize,
    /// **The signal.** Did the interaction CHANGE anything in each engine? A click that changes
    /// Chromium's document and changes nothing in Manuk is a dead interaction — the exact defect a
    /// render-only gate cannot see.
    pub chrome_changed: usize,
    pub manuk_changed: usize,
}

impl InteractionResult {
    /// Coverage of the post-interaction document.
    pub fn coverage(&self) -> f64 {
        if self.probed_after == 0 {
            return 1.0;
        }
        (self.probed_after - self.missing_after) as f64 / self.probed_after as f64
    }

    /// **The CRITICAL condition**: Chromium's document responded and Manuk's did not. Not "responded
    /// differently" — did not respond *at all*. That is a dead control, and no amount of rendering
    /// fidelity compensates for it.
    pub fn dead_interaction(&self) -> bool {
        self.chrome_changed > 0 && self.manuk_changed == 0
    }
}

/// Count the ids whose box differs between two snapshots — "did anything happen?".
pub fn changed_boxes(
    before: &std::collections::HashMap<String, [i32; 4]>,
    after: &std::collections::HashMap<String, [i32; 4]>,
) -> usize {
    let mut n = 0;
    for (id, b) in before {
        match after.get(id) {
            None => n += 1, // it disappeared — that is a change
            Some(a) => {
                if (0..4).any(|i| (a[i] - b[i]).abs() > 2) {
                    n += 1;
                }
            }
        }
    }
    // Elements that appeared are changes too (a menu opening is mostly *new* boxes).
    n + after.keys().filter(|id| !before.contains_key(*id)).count()
}

/// Print the report + the gate verdict.
pub fn report(rows: &[InteractionResult], floor: f64) -> bool {
    println!("\n=== G5 · INTERACTION PARITY vs Chromium ===\n");
    println!(
        "{:<28} {:>9} {:>8} {:>7} {:>9} {:>8}",
        "scenario", "COVERAGE", "missing", "moved", "chrome Δ", "manuk Δ"
    );
    let mut ok = true;
    for r in rows {
        let dead = r.dead_interaction();
        if dead || r.coverage() < floor {
            ok = false;
        }
        println!(
            "{:<28} {:>8.1}% {:>8} {:>7} {:>9} {:>8}{}",
            r.name,
            r.coverage() * 100.0,
            r.missing_after,
            r.moved_after,
            r.chrome_changed,
            r.manuk_changed,
            if dead { "   <-- DEAD INTERACTION (CRITICAL)" } else { "" }
        );
    }
    let n = rows.len().max(1) as f64;
    let mean = rows.iter().map(|r| r.coverage()).sum::<f64>() / n;
    let dead = rows.iter().filter(|r| r.dead_interaction()).count();
    println!("\nMEAN POST-INTERACTION COVERAGE: {:.1}%", mean * 100.0);
    println!(
        "DEAD INTERACTIONS: {dead}   (Chromium's document responded; Manuk's did not — ADR-007 CRITICAL)"
    );
    println!(
        "\n`chrome Δ` / `manuk Δ` are how many boxes MOVED. They are the honest signal: a click that\n\
         changes Chromium's document and changes nothing in Manuk is a dead control, and no amount\n\
         of rendering fidelity compensates for it.\n"
    );
    ok
}

/// Write a before/after composite so the interaction can be *looked at*, not just scored.
pub fn write_pair(before: &Path, after: &Path, dest: &Path) -> Result<()> {
    crate::fidelity::write_side_by_side(before, after, dest)
}
