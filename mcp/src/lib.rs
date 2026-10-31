//! manuk-mcp: the agent browser as an MCP server. `session` holds the tool logic against a
//! [`backend::Backend`]; `server` is the thin rmcp stdio adapter.

pub mod backend;
pub mod cdp;
pub mod downloads;
pub use manuk_agent::confine;
pub mod hub;
pub mod isolation;
pub mod policy;
pub mod server;
pub mod session;
pub mod taint;

pub use backend::Backend;
pub use session::{Output, Session};
