fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| a != "--deterministic")
        .collect();
    if args.len() != std::env::args().len() - 1 {
        manuk_js::determinism::enable();
    }
    let opts = manuk_mcp::isolation::Options::parse(args)?;
    let _held = manuk_mcp::isolation::apply(&opts)?;
    manuk_mcp::server::run_stdio(1280, 800)
}
