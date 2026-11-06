use manuk_pagestore::*;

fn n(role: &str, name: &str, kids: Vec<PNode>) -> PNode {
    PNode {
        role: role.into(),
        name: name.into(),
        state: String::new(),
        value: String::new(),
        rect: Some([0, 0, 10, 2]),
        children: kids,
    }
}
fn footer() -> PNode {
    n(
        "contentinfo",
        "Footer",
        (0..40)
            .map(|i| n("link", &format!("Footer link {i}"), vec![]))
            .collect(),
    )
}
fn page(items: &[&str]) -> PNode {
    let list = n(
        "list",
        "Results",
        items
            .iter()
            .map(|i| n("listitem", i, vec![n("link", i, vec![])]))
            .collect(),
    );
    n(
        "document",
        "Shop",
        vec![n("main", "", vec![list]), footer()],
    )
}
fn open() -> (tempdir::D, PageStore) {
    let d = tempdir::D::new();
    let s = PageStore::open(&d.0.join("p.redb")).unwrap();
    (d, s)
}
mod tempdir {
    pub struct D(pub std::path::PathBuf);
    impl D {
        pub fn new() -> D {
            let p = std::env::temp_dir().join(
                format!(
                    "pagestore-{}-{:?}",
                    std::process::id(),
                    std::thread::current().id()
                )
                .replace(['(', ')'], ""),
            );
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            D(p)
        }
    }
    impl Drop for D {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[test]
fn unchanged_footer_stored_once() {
    let (_d, s) = open();
    let a = s.put("shop.test", &page(&["Apple", "Pear"])).unwrap();
    let before = s.node_count().unwrap();
    let b = s
        .put("shop.test", &page(&["Apple", "Pear", "Plum"]))
        .unwrap();
    let after = s.node_count().unwrap();
    // footer = 1 + 40 nodes, all reused on the second visit
    assert!(b.reused_nodes >= 41, "reused {}", b.reused_nodes);
    // new rows: listitem+link for Plum, the new list, main, document = 5
    assert_eq!(after - before, 5, "new rows {}", after - before);
    assert_eq!(b.new_nodes, 5);
    assert_ne!(a.root, b.root);
    // identical revisit adds nothing
    let c = s
        .put("shop.test", &page(&["Apple", "Pear", "Plum"]))
        .unwrap();
    assert_eq!(c.new_nodes, 0);
    assert_eq!(c.root, b.root);
}

#[test]
fn diff_is_o_changed() {
    let (_d, s) = open();
    let a = s.put("shop.test", &page(&["Apple", "Pear"])).unwrap();
    let b = s
        .put("shop.test", &page(&["Apple", "Pear", "Plum"]))
        .unwrap();
    let (ch, reads) = s.diff("shop.test", &a.root, &b.root).unwrap();
    assert!(
        ch.contains(&Change::Added("listitem \"Plum\"".into())),
        "{ch:?}"
    );
    // the 41-node footer was never read: 2 reads per differing path node + the new subtree
    assert!(reads < 20, "reads {reads}");
    let (none, r0) = s.diff("shop.test", &b.root, &b.root).unwrap();
    assert!(none.is_empty() && r0 == 0);
}

#[test]
fn handles_survive_remount() {
    let (_d, s) = open();
    let t1 = page(&["Apple", "Pear"]);
    let h1 = s.handles("shop.test", &t1).unwrap();
    // re-mount: list rebuilt with a new item in front; existing items keep their numbers
    let t2 = page(&["Apple", "Pear"]);
    let h2 = s.handles("shop.test", &t2).unwrap();
    assert_eq!(h1, h2);
    let t3 = page(&["Fig", "Apple", "Pear"]);
    let h3: std::collections::HashMap<_, _> =
        s.handles("shop.test", &t3).unwrap().into_iter().collect();
    for (k, h) in &h1 {
        if k.contains("Apple") || k.contains("Pear") {
            assert_eq!(h3[k], *h, "{k}");
        }
    }
    let fig = h3
        .iter()
        .find(|(k, _)| k.starts_with("listitem|Fig"))
        .unwrap()
        .1;
    assert!(
        h1.iter().all(|(_, h)| h != fig),
        "new node must get a fresh number"
    );
}

#[test]
fn recall_returns_last_tree_and_age() {
    let (_d, s) = open();
    assert!(s.recall("shop.test").unwrap().is_none());
    s.put("shop.test", &page(&["Apple"])).unwrap();
    let last = page(&["Apple", "Pear"]);
    s.put("shop.test", &last).unwrap();
    let r = s.recall("shop.test").unwrap().unwrap();
    assert_eq!(r.seq, 2);
    assert_eq!(r.tree, last);
    assert!(r.age_secs < 5);
    assert!(s.recall("other.test").unwrap().is_none());
}

#[test]
fn dictionary_trained_and_edges_recorded() {
    let (_d, s) = open();
    let big = n(
        "document",
        "Big",
        (0..200)
            .map(|i| n("link", &format!("Item number {i} in the catalogue"), vec![]))
            .collect(),
    );
    s.put("big.test", &big).unwrap();
    assert_eq!(s.recall("big.test").unwrap().unwrap().tree, big);
    let a = s.put("shop.test", &page(&["A"])).unwrap();
    let b = s.put("shop.test", &page(&["A", "B"])).unwrap();
    s.record_edge(&a.root, "click h3 'Add'", &b.root).unwrap();
    assert_eq!(
        s.edges(&a.root).unwrap(),
        vec![("click h3 'Add'".to_string(), b.root.clone())]
    );
    assert!(s.edges(&b.root).unwrap().is_empty());
}
