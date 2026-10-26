fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--confine-selftest") {
        return selftest(&args[1..]);
    }
    let before = args.len();
    args.retain(|a| a != "--deterministic");
    if args.len() != before {
        manuk_js::determinism::enable();
    }
    let before = args.len();
    args.retain(|a| a != "--no-confine");
    let confine = args.len() == before;
    // `--allow-write <dir>`: operator-granted extra writable dirs (e.g. where `storage_state export`
    // may put files). Everything outside the cache dir, the profile and these is read-only.
    let mut allow_write = Vec::new();
    while let Some(i) = args.iter().position(|a| a == "--allow-write") {
        anyhow::ensure!(i + 1 < args.len(), "--allow-write needs a directory");
        allow_write.push(std::path::PathBuf::from(args.remove(i + 1)));
        args.remove(i);
    }
    // Agent mode runs untrusted pages: no JIT (all 12 recent SpiderMonkey CVEs were JIT/wasm).
    manuk_js::hardening::enable_jitless();
    let opts = manuk_mcp::isolation::Options::parse(args)?;
    let _held = manuk_mcp::isolation::apply(&opts)?;
    if confine {
        let cache = manuk_mcp::confine::cache_dir();
        let mut g = manuk_mcp::confine::Grants::default();
        g.write.push(cache);
        g.write.extend(allow_write);
        g.write.extend(opts.profile.clone());
        // The action log directory is an explicit operator choice (server.rs reads the same var).
        g.write
            .extend(std::env::var_os("MANUK_ACTLOG_DIR").map(std::path::PathBuf::from));
        g.read.extend(opts.storage_state.clone());
        g.read.extend(std::env::current_dir().ok());
        if let manuk_mcp::confine::Outcome::Unsupported(why) = manuk_mcp::confine::confine(&g) {
            eprintln!("manuk-mcp: WARNING: OS confinement unavailable ({why}); running unconfined");
        }
    }
    manuk_mcp::server::run_stdio(1280, 800)
}

/// `--confine-selftest <outside-file> <inside-dir> [--no-confine]`: confine, then report what the
/// process can still do. The gate test runs this and reads the lines.
fn selftest(a: &[String]) -> anyhow::Result<()> {
    let (outside, inside) = (&a[0], &a[1]);
    if !a.iter().any(|x| x == "--no-confine") {
        let g = manuk_mcp::confine::Grants {
            read: vec![],
            write: vec![inside.into()],
        };
        println!("confine={:?}", manuk_mcp::confine::confine(&g));
    }
    let w = |p: &str| std::fs::write(p, b"x").is_ok();
    println!("write_outside={}", w(outside));
    println!("write_inside={}", w(&format!("{inside}/ok")));
    println!(
        "exec={}",
        std::process::Command::new("/bin/true").status().is_ok()
    );
    println!("read_usr={}", std::fs::read_dir("/usr").is_ok());
    println!(
        "read_home={}",
        std::env::var("HOME")
            .map(|h| std::fs::read_dir(h).is_ok())
            .unwrap_or(false)
    );
    Ok(())
}
