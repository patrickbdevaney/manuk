fn main() -> anyhow::Result<()> {
    let opts = manuk_mcp::isolation::Options::parse(std::env::args().skip(1))?;
    let _held = manuk_mcp::isolation::apply(&opts)?;
    manuk_mcp::server::run_stdio(1280, 800)
}
