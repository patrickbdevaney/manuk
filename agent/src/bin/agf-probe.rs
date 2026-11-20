//! `agf-probe` — the manuk half of W6.R1 (actionable-geometry fidelity, AGF).
//!
//! For one URL at 1024x768, prints one TSV row per actionable, named target (the same set
//! `drive-probe` scores: `drivability::targets`):
//!
//! ```text
//!   role  name  x  y  w  h  hit_in_subtree  hit_role  hit_name
//! ```
//!
//! `x,y,w,h` is the target's bbox in manuk's layout (`-` when it has none). `hit_in_subtree` is 1 when
//! the top hit-test element at the bbox centre is the target or one of its descendants (the same
//! criterion as `drivability::classify`). The Chrome half, the geometry comparison and the miss
//! survey live in `bench/agent-tasks/r1_agf.py`; this binary asks nothing of Chrome.
//!
//! ```text
//!   agf-probe <url>
//! ```

use anyhow::{anyhow, Result};
use manuk_agent::drivability::{self, targets};
use manuk_agent::AgentBrowser;

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[tokio::main]
async fn main() -> Result<()> {
    let url = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("usage: agf-probe <url>"))?;
    let mut browser = AgentBrowser::new(1024, 768);
    browser.navigate(&url).await?;
    let tree = browser.a11y_tree()?;
    for t in targets(&tree) {
        let (geom, hit) = match t.bbox {
            Some(b) if b.width > 0.0 && b.height > 0.0 => {
                let g = format!("{:.2}\t{:.2}\t{:.2}\t{:.2}", b.x, b.y, b.width, b.height);
                let (cx, cy) = (b.x + b.width / 2.0, b.y + b.height / 2.0);
                let h = match tree.hit_test(cx, cy) {
                    Some(hit) => format!(
                        "{}\t{}\t{}",
                        drivability::in_subtree(t, hit.node) as u8,
                        hit.role.as_str(),
                        clean(&hit.name)
                    ),
                    None => "0\t-\t-".to_string(),
                };
                (g, h)
            }
            _ => ("-\t-\t-\t-".to_string(), "0\t-\t-".to_string()),
        };
        println!("{}\t{}\t{}\t{}", t.role.as_str(), clean(&t.name), geom, hit);
    }
    Ok(())
}
