//! `MANUK_NET_TRACE=<file>` — a per-request phase trace for the turnaround profile (PERF.1).
//!
//! Off unless the env var names a file, in which case every network event appends one TSV line
//! `t_us <TAB> kind <TAB> fields...` (t_us is microseconds since the first event of the process):
//!
//! ```text
//!   epoch    unix_us                            wall clock at t_us=0 (first line of the file)
//!   dns      host  dur_us                       resolver call (getaddrinfo)
//!   connect  host  dur_us                       DNS + TCP + TLS for one NEW connection
//!   req      url   start_us ttfb_us status http_version content_length
//!   body     url   bytes                        decoded body fully read (t_us = end of body)
//! ```
//!
//! A request with no `connect` line before its `req` reused a pooled connection. Disabled, each
//! hook costs one `OnceLock` read.

use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

fn sink() -> &'static Option<Mutex<std::fs::File>> {
    static SINK: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();
    SINK.get_or_init(|| {
        let path = std::env::var_os("MANUK_NET_TRACE")?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()
            .map(Mutex::new)
    })
}

pub fn enabled() -> bool {
    sink().is_some()
}

/// Microseconds since the first call in this process (the trace's own epoch).
pub fn now_us() -> u64 {
    t0().elapsed().as_micros() as u64
}

fn t0() -> &'static Instant {
    static T0: OnceLock<Instant> = OnceLock::new();
    T0.get_or_init(|| {
        if let Some(f) = sink() {
            if let (Ok(mut f), Ok(d)) = (
                f.lock(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH),
            ) {
                let _ = writeln!(f, "0\tepoch\t{}", d.as_micros());
            }
        }
        Instant::now()
    })
}

pub fn emit(kind: &str, fields: &str) {
    if let Some(f) = sink() {
        let t = now_us(); // before the lock: the first call writes the epoch line under it
        if let Ok(mut f) = f.lock() {
            let _ = writeln!(f, "{t}\t{kind}\t{fields}");
        }
    }
}
