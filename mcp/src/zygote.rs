//! W4.E4: a warm pre-forked process pool. `manuk-mcp --zygote <socket>` is a single-threaded parent
//! that does the thread-free part of start-up once (binary mapped and faulted in, tool schemas
//! built), then `fork()`s one child per connection. The child's stdin/stdout are the connection, and
//! it runs the ordinary stdio server, so a session is exactly what `manuk-mcp` is, minus exec,
//! dynamic linking and start-up. Every session is its own process: cookies, storage and the JS
//! realm are never shared (the parent holds no session state and the child sets its own isolation).
//! The tokio runtimes, the session thread and the SpiderMonkey helper threads all start in the
//! child; the parent asserts it has exactly one thread at every fork.

use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixListener;
use std::path::Path;

use anyhow::{bail, Context, Result};

/// Threads in this process, from /proc (the kernel's count, not ours).
pub fn thread_count() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("Threads:").map(|v| v.trim().parse().ok()))
                .flatten()
        })
        .unwrap_or(0)
}

/// Serve forever in the parent; return `Ok(())` only in a forked child, with fd 0/1 = the session.
pub fn serve(sock: &Path) -> Result<()> {
    let _ = std::fs::remove_file(sock);
    let listener = UnixListener::bind(sock).with_context(|| format!("bind {}", sock.display()))?;
    // Children are reaped by the kernel; a session ending must not leave a zombie in the pool.
    unsafe { libc::signal(libc::SIGCHLD, libc::SIG_IGN) };
    // The thread-free warm-up: build the tool schemas once (touches the rmcp/serde paths).
    let _ = crate::server::tools();
    eprintln!("manuk-mcp: zygote ready on {}", sock.display());
    loop {
        let (conn, _) = match listener.accept() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => bail!("accept: {e}"),
        };
        let n = thread_count();
        if n != 1 {
            bail!("zygote has {n} threads at fork; refusing (fork safety)");
        }
        match unsafe { libc::fork() } {
            -1 => eprintln!(
                "manuk-mcp: fork failed: {}",
                std::io::Error::last_os_error()
            ),
            0 => {
                let fd = conn.as_raw_fd();
                unsafe {
                    libc::dup2(fd, 0);
                    libc::dup2(fd, 1);
                    libc::signal(libc::SIGCHLD, libc::SIG_DFL);
                }
                drop(conn);
                // The child must not keep the pool's listening socket open.
                drop(listener);
                return Ok(());
            }
            _ => drop(conn),
        }
    }
}
