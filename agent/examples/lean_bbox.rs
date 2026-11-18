//! W4.E2.a.a.a probe: dump what a tool-level agent can see — per-node bbox for links/images/headings,
//! content height, what a click at each link's centre lands on, and the observe() line count —
//! under the profile in `MANUK_AGENT_PROFILE`. Usage: `lean_bbox <url>`. Output is TSV-ish, diffable.
fn walk(n: &manuk_a11y::A11yNode, out: &mut Vec<String>) {
    if let Some(b) = n.bbox {
        let r = n.role.as_str();
        if matches!(r, "link" | "img" | "image" | "heading" | "paragraph") {
            out.push(format!(
                "node\t{r}\t{:?}\t{:.1}\t{:.1}\t{:.1}\t{:.1}",
                n.name, b.x, b.y, b.width, b.height
            ));
        }
    }
    for c in &n.children {
        walk(c, out);
    }
}
fn main() {
    let url = std::env::args().nth(1).expect("usage: lean_bbox <url>");
    manuk_net::runtime().block_on(async {
        let mut b = manuk_agent::AgentBrowser::new(1280, 800);
        b.navigate(&url).await.expect("navigate");
        let tree = b.a11y_tree().expect("tree");
        let mut lines = Vec::new();
        walk(&tree, &mut lines);
        let o = b.observe().expect("observe");
        println!("content_height\t{:.1}", o.content_height);
        println!("sem_lines\t{}", o.semantics.len());
        for l in lines {
            println!("{l}");
        }
        for l in &o.semantics {
            println!("sem\t{l}");
        }
        // Landing: a click point at each link's centre, resolved by the same hit test `click_at` uses.
        fn links(n: &manuk_a11y::A11yNode, v: &mut Vec<(String, f32, f32)>) {
            if n.role.as_str() == "link" {
                if let Some(b) = n.bbox {
                    v.push((n.name.clone(), b.x + b.width / 2.0, b.y + b.height / 2.0));
                }
            }
            for c in &n.children {
                links(c, v);
            }
        }
        let mut v = Vec::new();
        links(&tree, &mut v);
        for (name, x, y) in v {
            let hit = tree
                .hit_test(x, y)
                .map(|h| format!("{} {:?}", h.role.as_str(), h.name))
                .unwrap_or("none".into());
            println!("landing\t{name:?}\t({x:.1},{y:.1})\t{hit}");
        }
    });
}
