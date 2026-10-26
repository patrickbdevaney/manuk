//! OS confinement of the agent process (R1.6). Linux landlock, applied in `main` before any thread
//! exists (a landlock domain is inherited by threads created *after* `restrict_self`; there is no
//! TSYNC in the v1 ABI, so "before threads" is the only correct order).
//!
//! What the page-reading process may do afterwards:
//! * **read** the system roots (`/usr`, `/lib*`, `/etc`, `/proc`, `/sys`, `/dev`, `/opt`, `/run`,
//!   `/bin`, `/sbin`, `/tmp`), the cache dir, the profile dir, the files it was explicitly handed and
//!   the working directory. `$HOME` is *not* readable otherwise: no `~/.ssh`, no browser profiles.
//! * **write** the cache dir and the profile dir (when `--profile`), plus `/dev/null`.
//! * **exec** nothing (`LANDLOCK_ACCESS_FS_EXECUTE` is handled and never granted).
//!
//! Not covered (landlock v1 is filesystem-only): network (the egress guard in `manuk-net` owns it),
//! `ptrace`, signals. Unsupported kernel => a loud warning, not a failure: the process still has the
//! jitless engine, and refusing to start would make the browser unusable on old kernels.

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

const SYS_CREATE_RULESET: libc::c_long = 444;
const SYS_ADD_RULE: libc::c_long = 445;
const SYS_RESTRICT_SELF: libc::c_long = 446;

const EXECUTE: u64 = 1 << 0;
const WRITE_FILE: u64 = 1 << 1;
const READ_FILE: u64 = 1 << 2;
const READ_DIR: u64 = 1 << 3;
const REMOVE_DIR: u64 = 1 << 4;
const REMOVE_FILE: u64 = 1 << 5;
const HANDLED: u64 = 0x1FFF;
const READ: u64 = READ_FILE | READ_DIR;
const WRITE: u64 = HANDLED & !EXECUTE;

#[repr(C)]
struct RulesetAttr {
    handled_access_fs: u64,
}
#[repr(C, packed)]
struct PathBeneath {
    allowed_access: u64,
    parent_fd: i32,
}

/// Where the engine's disk cache lives and the only place (with the profile) the process may write.
pub fn cache_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("MANUK_CACHE_DIR") {
        PathBuf::from(d)
    } else if let Some(d) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(d).join("manuk")
    } else if let Some(h) = std::env::var_os("HOME") {
        PathBuf::from(h).join(".cache/manuk")
    } else {
        PathBuf::from("/tmp/manuk-cache")
    }
}

#[derive(Default)]
pub struct Grants {
    pub read: Vec<PathBuf>,
    pub write: Vec<PathBuf>,
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Enforced,
    Unsupported(String),
}

fn add(ruleset: i32, path: &Path, access: u64) -> bool {
    let Ok(c) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
    if fd < 0 {
        return false; // a missing optional root (e.g. /lib64) is simply not granted
    }
    // Directory-only rights on a regular file are EINVAL; narrow them.
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let is_dir =
        unsafe { libc::fstat(fd, &mut st) } == 0 && (st.st_mode & libc::S_IFMT) == libc::S_IFDIR;
    let access = if is_dir {
        access
    } else {
        access & (EXECUTE | WRITE_FILE | READ_FILE)
    };
    let rule = PathBeneath {
        allowed_access: access,
        parent_fd: fd,
    };
    let r = unsafe { libc::syscall(SYS_ADD_RULE, ruleset, 1, &rule as *const _, 0) };
    unsafe { libc::close(fd) };
    r == 0
}

/// Apply the ruleset to the calling thread (and every thread it creates from now on).
pub fn confine(g: &Grants) -> Outcome {
    let attr = RulesetAttr {
        handled_access_fs: HANDLED,
    };
    let fd = unsafe {
        libc::syscall(
            SYS_CREATE_RULESET,
            &attr as *const _,
            std::mem::size_of::<RulesetAttr>(),
            0,
        )
    } as i32;
    if fd < 0 {
        return Outcome::Unsupported(format!(
            "landlock_create_ruleset: {}",
            std::io::Error::last_os_error()
        ));
    }
    for root in [
        "/usr", "/lib", "/lib64", "/lib32", "/etc", "/proc", "/sys", "/dev", "/opt", "/run",
        "/bin", "/sbin", "/tmp",
    ] {
        add(fd, Path::new(root), READ);
    }
    add(fd, Path::new("/dev/null"), WRITE_FILE | READ_FILE);
    for p in &g.read {
        add(fd, p, READ);
    }
    for p in &g.write {
        let _ = std::fs::create_dir_all(p);
        add(fd, p, WRITE | READ);
    }
    let rc = unsafe {
        libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0);
        libc::syscall(SYS_RESTRICT_SELF, fd, 0)
    };
    unsafe { libc::close(fd) };
    if rc != 0 {
        return Outcome::Unsupported(format!(
            "landlock_restrict_self: {}",
            std::io::Error::last_os_error()
        ));
    }
    Outcome::Enforced
}

/// Jitless engine + landlock for a headless agent binary (R1.6.a). Call first thing in a plain
/// synchronous `main`, before any thread exists (so not under `#[tokio::main]`). `--no-confine` is
/// stripped from `args` and turns the OS half off (the jitless half stays); returns whether confined.
pub fn harden_process(args: &mut Vec<String>, extra: Grants) -> bool {
    let before = args.len();
    args.retain(|a| a != "--no-confine");
    let confined = args.len() == before;
    manuk_js::hardening::enable_jitless();
    if confined {
        let mut g = extra;
        g.write.push(cache_dir());
        g.read.extend(std::env::current_dir().ok());
        if let Outcome::Unsupported(why) = confine(&g) {
            eprintln!("WARNING: OS confinement unavailable ({why}); running unconfined");
        }
    }
    confined
}

/// `--confine-selftest <outside-file> <inside-dir> [--no-confine]`: confine, then report what the
/// process can still do. The gate tests run this on every agent binary and read the lines.
pub fn selftest(a: &[String]) -> anyhow::Result<()> {
    anyhow::ensure!(
        a.len() >= 2,
        "--confine-selftest <outside-file> <inside-dir>"
    );
    let (outside, inside) = (&a[0], &a[1]);
    if !a.iter().any(|x| x == "--no-confine") {
        let g = Grants {
            read: vec![],
            write: vec![inside.into()],
        };
        manuk_js::hardening::enable_jitless();
        println!("confine={:?}", confine(&g));
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
    println!("jitless={}", manuk_js::hardening::jitless());
    Ok(())
}
