fn main() -> anyhow::Result<()> {
    manuk_mcp::server::run_stdio(1280, 800)
}
