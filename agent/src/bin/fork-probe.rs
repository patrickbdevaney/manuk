//! `fork-probe <fixture.html> <children> <mode>` — R3.4. Loads a page, quiesces, then `fork()`s
//! `<children>` children from the loaded page process. Each child runs JS that is different per child
//! (sets a distinct token, allocates, forces a GC), reads its a11y tree, and reports
//! `idx digest rss_kb private_dirty_kb fork_us` over a pipe. The parent prints one TSV line per child.
//!
//! Modes: `naive` = fork with whatever threads are live; `quiesced` = after settle, assert the thread
//! census first (refuse when a thread outside the known set is alive).
use anyhow::{bail, Result};
use manuk_agent::AgentBrowser;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::io::FromRawFd;
use std::time::{Duration, Instant};

fn census() -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir("/proc/self/task") {
        for e in rd.flatten() {
            let name = std::fs::read_to_string(e.path().join("comm")).unwrap_or_default();
            // Collapse numbered pool workers: "DOM Worker" and "tokio-runtime-w" stay as-is.
            *m.entry(name.trim().to_string()).or_default() += 1;
        }
    }
    m
}

fn kb(field: &str, file: &str) -> u64 {
    std::fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.strip_prefix(field))
        .filter_map(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
        .sum()
}

fn digest(s: &str) -> String {
    // FNV-1a 64: deterministic, no deps.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn tree_text(b: &AgentBrowser) -> String {
    match b.a11y_tree_with_text() {
        Ok(t) => format!("{t:?}"),
        Err(e) => format!("ERR {e}"),
    }
}

/// What a child does: a per-index mutation, allocation + GC, then an observation.
fn child_work(b: &mut AgentBrowser, idx: usize) -> (String, String) {
    let js = format!(
        "window.__who='child{idx}';document.getElementById('b').click();\
         var a=[];for(var i=0;i<20000;i++)a.push({{i:i,s:'x'+i}});a=null;\
         if(typeof gc==='function')gc();\
         var o=document.getElementById('out').textContent;o"
    );
    let (_, errs) = b.probe_eval(&js);
    let tree = tree_text(b);
    // Entropy read AFTER the tree is captured: probe_eval returns its value through a `#__probe` node.
    let (rand, rerrs) = b.probe_eval(
        "var p=document.createElement('pre');p.id='__probe';document.body.appendChild(p);\
         p.textContent=String(Math.random())+'/'+(typeof crypto==='object'?crypto.getRandomValues(new Uint32Array(2)).join(','):'nocrypto')",
    );
    let rand = if rand.is_empty() { format!("ERR:{}", rerrs.join(";").replace('\t', " ")) } else { rand };
    (format!("errs={}|{}", errs.len(), tree), rand)
}

/// Open sockets in this process, from /proc/self/fd (the "0 open sockets before fork" check).
fn open_sockets() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .map(|rd| {
            rd.flatten()
                // fds 0-2 are the harness's stdio (a socket under some launchers), not the page's.
                .filter(|e| e.file_name().to_string_lossy().parse::<u32>().map(|n| n > 2).unwrap_or(true))
                .filter(|e| {
                    std::fs::read_link(e.path())
                        .map(|t| t.to_string_lossy().starts_with("socket:"))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

/// Local TCP ports of this process's sockets (distinct port = distinct connection = distinct handshake).
fn local_ports() -> Vec<u16> {
    let inodes: Vec<String> = std::fs::read_dir("/proc/self/fd")
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| std::fs::read_link(e.path()).ok())
                .filter_map(|t| {
                    let t = t.to_string_lossy().to_string();
                    t.strip_prefix("socket:[").map(|r| r.trim_end_matches(']').to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    let mut ports = Vec::new();
    for f in ["/proc/net/tcp", "/proc/net/tcp6"] {
        for l in std::fs::read_to_string(f).unwrap_or_default().lines().skip(1) {
            let c: Vec<&str> = l.split_whitespace().collect();
            if c.len() > 9 && inodes.iter().any(|i| i == c[9]) {
                if let Some(p) = c[1].rsplit(':').next().and_then(|p| u16::from_str_radix(p, 16).ok()) {
                    ports.push(p);
                }
            }
        }
    }
    ports
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        bail!("usage: fork-probe <fixture.html> <children> <naive|quiesced>");
    }
    let n: usize = args[1].parse()?;
    let net_url = args.get(3).cloned();
    let mode = args[2].as_str();
    // `ref:<idx>`: no fork — run child `idx`'s work in a fresh process and print its digest, so the
    // gate can check a forked child's digest against an independent, never-forked run.
    let ref_idx: Option<usize> = mode.strip_prefix("ref:").and_then(|v| v.parse().ok());
    let path = std::fs::canonicalize(&args[0])?;
    let mk_rt = || tokio::runtime::Builder::new_current_thread().enable_all().build();
    let rt = mk_rt()?;
    if mode == "quiesced" {
        // Must precede the first SpiderMonkey runtime.
        manuk_js::forkpool::enable();
    }
    let mut b = AgentBrowser::new(1280, 900);
    rt.block_on(async {
        // With a net URL the parent first uses the network (pooled connections must not survive).
        if let Some(u) = &net_url {
            b.navigate(u).await?;
            b.settle().await;
        }
        b.navigate(&format!("file://{}", path.display())).await?;
        b.settle().await;
        Ok::<_, anyhow::Error>(())
    })?;
    // The page's own connections live on this runtime's tasks: dropping it closes them.
    drop(rt);
    // Mutant for the socket gate: a live socket at fork time.
    let _leak = std::env::var_os("FORK_PROBE_KEEP_SOCKET")
        .map(|_| std::os::unix::net::UnixStream::pair().expect("socketpair"));
    let socks = open_sockets();
    eprintln!("open sockets before fork: {socks} ports={:?}", local_ports());
    if socks > 0 {
        for e in std::fs::read_dir("/proc/self/fd").unwrap().flatten() {
            if let Ok(t) = std::fs::read_link(e.path()) {
                if t.to_string_lossy().starts_with("socket:") {
                    eprintln!("  fd {:?} -> {}", e.file_name(), t.display());
                }
            }
        }
    }
    if let Some(idx) = ref_idx {
        let (out, _) = child_work(&mut b, idx);
        println!("{idx}\t{}", digest(&out));
        return Ok(());
    }
    let parent_before = digest(&tree_text(&b));
    let cen = census();
    eprintln!("census after load: {cen:?}");
    if mode == "quiesced" {
        let total: usize = cen.values().sum();
        eprintln!("threads at fork: {total}");
    }
    let mut kids = Vec::new();
    // Quiesce ONCE: after this the process has one thread, and every fork below is from that state.
    let q = if mode == "quiesced" {
        let q = manuk_js::forkpool::quiesce();
        let t = manuk_js::forkpool::thread_count();
        eprintln!("threads at fork: {t}");
        if t != 1 {
            bail!("refusing to fork with {t} threads");
        }
        Some(q)
    } else {
        None
    };
    for idx in 0..n {
        let mut fds = [0i32; 2];
        unsafe { libc::pipe(fds.as_mut_ptr()) };
        let t0 = Instant::now();
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            unsafe { libc::close(fds[0]) };
            if let Some(q) = &q {
                manuk_js::forkpool::resume(q);
                if std::env::var_os("FORK_PROBE_NO_RESEED").is_none() {
                    manuk_js::forkpool::mark_child_for_reseed();
                }
            }
            let fork_us = t0.elapsed().as_micros();
            let rss = kb("VmRSS:", "/proc/self/status");
            let (out, rand) = child_work(&mut b, idx);
            // FORK_PROBE_SPIN: the child runs an infinite loop under MANUK_MAX_DRAIN_MS; the script
            // deadline (the watchdog thread, which a fork does not carry over) must cut it.
            let mut spin = String::new();
            if std::env::var_os("FORK_PROBE_SPIN").is_some() {
                let t = Instant::now();
                b.probe_eval("while(true){}");
                spin = format!("|spin_cut_ms={}", t.elapsed().as_millis());
            }
            let rand = format!("{rand}{spin}");
            // With a net URL, each child opens its own connection(s) on a fresh runtime.
            let mut net = String::from("-");
            if let Some(u) = &net_url {
                let rt = mk_rt().unwrap();
                // A distinct query string per child: the parent's in-process cache must not answer.
                let u = format!("{u}?child={idx}");
                let r = rt.block_on(async {
                    b.navigate(&u).await?;
                    b.settle().await;
                    Ok::<_, anyhow::Error>(())
                });
                let ports = local_ports();
                net = format!("{}:{:?}{}", r.is_ok() && !ports.is_empty(), ports, r.err().map(|e| format!(":{e}")).unwrap_or_default());
            }
            let pd = kb("Private_Dirty:", "/proc/self/smaps");
            let line = format!(
                "{idx}\t{}\t{rss}\t{pd}\t{fork_us}\t{}\t{rand}\t{net}\n",
                digest(&out),
                out.contains(&format!("clicked child{idx}"))
            );
            let mut w = unsafe { std::fs::File::from_raw_fd(fds[1]) };
            let _ = w.write_all(line.as_bytes());
            drop(w);
            unsafe { libc::_exit(0) };
        }
        let fork_us = t0.elapsed().as_micros();
        unsafe { libc::close(fds[1]) };
        kids.push((idx, pid, fds[0], fork_us));
    }
    if let Some(q) = &q {
        manuk_js::forkpool::resume(q);
    }
    println!("idx\tdigest\trss_kb\tprivate_dirty_kb\tchild_fork_us\town_token\trand\tnet\tparent_fork_us\tstatus");
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut digests = Vec::new();
    let mut rands = Vec::new();
    let mut net_ok = 0;
    let mut all_ports: Vec<String> = Vec::new();
    let (mut ok, mut hung) = (0, 0);
    for (idx, pid, fd, fork_us) in kids {
        let mut f = unsafe { std::fs::File::from_raw_fd(fd) };
        // Poll for exit (hang detection) before reading.
        let mut status = 0i32;
        let mut done = false;
        while Instant::now() < deadline {
            if unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) } == pid {
                done = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if !done {
            unsafe { libc::kill(pid, libc::SIGKILL) };
            unsafe { libc::waitpid(pid, &mut status, 0) };
            hung += 1;
            println!("{idx}\t-\t-\t-\t-\t-\t-\t-\t{fork_us}\tHUNG");
            continue;
        }
        let mut s = String::new();
        let _ = f.read_to_string(&mut s);
        let s = s.trim_end();
        if s.is_empty() {
            println!("{idx}\t-\t-\t-\t-\t-\t-\t-\t{fork_us}\tDIED(status={status})");
            continue;
        }
        let cols: Vec<&str> = s.split('\t').collect();
        digests.push(cols[1].to_string());
        rands.push(cols[6].split('/').next().unwrap_or("").to_string());
        if cols[7].starts_with("true") {
            net_ok += 1;
            all_ports.push(cols[7].to_string());
        }
        if cols[5] == "true" {
            ok += 1;
        }
        println!("{s}\t{fork_us}\tEXIT({status})");
    }
    let parent_after = digest(&tree_text(&b));
    digests.sort();
    let distinct = {
        let mut d = digests.clone();
        d.dedup();
        d.len()
    };
    rands.sort();
    rands.dedup();
    all_ports.sort();
    all_ports.dedup();
    eprintln!(
        "SUMMARY children={n} replied={} own_token={ok} distinct_digests={distinct} distinct_math_random={} net_ok={net_ok} distinct_conns={} sockets_at_fork={socks} hung={hung} parent_unchanged={}",
        digests.len(),
        rands.len(),
        all_ports.len(),
        parent_before == parent_after
    );
    Ok(())
}
