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

/// Fork-safety census: the forking thread alone, or - when the work runs on a big-stack thread the
/// process leader spawned - that thread plus the leader parked in `join` (a thread blocked in a
/// futex holds no allocator lock and is not copied into the child).
fn forkable() -> bool {
    match thread_count() {
        1 => true,
        2 => {
            let me = unsafe { libc::syscall(libc::SYS_gettid) } as u32;
            let pid = std::process::id();
            me != pid
                && std::fs::read_to_string(format!("/proc/self/task/{pid}/stat"))
                    .map(|s| s.rsplit(") ").next().is_some_and(|r| r.starts_with('S')))
                    .unwrap_or(false)
        }
        _ => false,
    }
}

/// A page loaded in the zygote and a quiesced SpiderMonkey, handed to a forked child.
pub struct Preloaded {
    pub browser: manuk_agent::AgentBrowser,
    quiesced: manuk_js::forkpool::Quiesced,
}

impl Preloaded {
    /// Child only, after confinement: restart the helper pool and watchdog, and make the next script
    /// entry reseed `Math.random`.
    pub fn wake(self) -> manuk_agent::AgentBrowser {
        manuk_js::forkpool::resume(&self.quiesced);
        manuk_js::forkpool::mark_child_for_reseed();
        self.browser
    }
}

/// R3.4.a: like [`serve`], but the parent first loads `url`, lets it settle, drops its runtime and
/// quiesces SpiderMonkey, so every forked session starts with that page already live. Returns in a
/// forked child with the page; the parent never returns. Must be called before any other
/// SpiderMonkey use, on the thread that will also run the session (the engine is thread-affine).
pub fn serve_after_load(sock: &Path, url: &str, w: u32, h: u32) -> Result<Preloaded> {
    manuk_js::forkpool::enable();
    let mut browser = crate::server::new_agent_browser(w, h);
    {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(async {
            browser.navigate(url).await?;
            browser.settle().await;
            anyhow::Ok(())
        })?;
        // The page's own connections live on this runtime's tasks: dropping it closes them.
    }
    let socks = std::fs::read_dir("/proc/self/fd")
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .parse::<u32>()
                        .map(|n| n > 2)
                        .unwrap_or(true)
                })
                .filter(|e| {
                    std::fs::read_link(e.path())
                        .map(|t| t.to_string_lossy().starts_with("socket:"))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0);
    if socks != 0 {
        bail!("zygote has {socks} open sockets after load; refusing (a forked session would share them)");
    }
    let quiesced = manuk_js::forkpool::quiesce();
    accept_loop(sock, true)?;
    Ok(Preloaded { browser, quiesced })
}

/// Serve forever in the parent; return `Ok(())` only in a forked child, with fd 0/1 = the session.
pub fn serve(sock: &Path) -> Result<()> {
    accept_loop(sock, false)
}

fn accept_loop(sock: &Path, preloaded: bool) -> Result<()> {
    let _ = std::fs::remove_file(sock);
    let listener = UnixListener::bind(sock).with_context(|| format!("bind {}", sock.display()))?;
    // Children are reaped by the kernel; a session ending must not leave a zombie in the pool.
    unsafe { libc::signal(libc::SIGCHLD, libc::SIG_IGN) };
    // The thread-free warm-up: build the tool schemas once (touches the rmcp/serde paths).
    let _ = crate::server::tools();
    // The system-font scan is plain data; do it here so no child repeats it.
    manuk_text::FontContext::warm_system_fonts();
    eprintln!("manuk-mcp: zygote ready on {}", sock.display());
    loop {
        let (conn, _) = match listener.accept() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => bail!("accept: {e}"),
        };
        let n = thread_count();
        if !(if preloaded { forkable() } else { n == 1 }) {
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
