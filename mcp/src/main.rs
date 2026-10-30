fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--confine-selftest") {
        return manuk_mcp::confine::selftest(&args[1..]);
    }
    let grants = match manuk_mcp::policy::parse_grants(&mut args) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("manuk-mcp: {e}");
            std::process::exit(2);
        }
    };
    if let Some(i) = args.iter().position(|a| a == "--backend") {
        anyhow::ensure!(
            args.get(i + 1).map(String::as_str) == Some("chrome"),
            "--backend takes `chrome`"
        );
        return manuk_mcp::server::run_stdio_chrome(1280, 800, grants);
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
    // Agent mode runs untrusted pages: baseline JIT tiers only, Ion/wasm-JIT off; `--jitless` = none.
    manuk_mcp::confine::set_jit_mode(&mut args);
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
    manuk_mcp::server::run_stdio(1280, 800, grants)
}
