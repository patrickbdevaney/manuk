//! Session isolation (W3.4). An agent session starts in an ephemeral in-memory cookie + storage
//! partition: it neither sees the human's logins nor leaves any behind. `--profile <dir>` opts into
//! a persistent profile, and one process owns a profile at a time (a second is refused); parallel
//! tasks are separate processes. Playwright storage-state JSON moves state in and out, and a
//! per-origin grant (logged) is the only way the human's cookies reach a session.

use std::fs::File;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};

#[derive(Default, Debug, PartialEq)]
pub struct Options {
    pub profile: Option<PathBuf>,
    pub storage_state: Option<PathBuf>,
    pub grant_origins: Vec<String>,
    /// Where the human's cookies live; defaults to the GUI's store.
    pub human_cookies: Option<PathBuf>,
}

impl Options {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Options> {
        let mut o = Options::default();
        let mut it = args.into_iter();
        while let Some(a) = it.next() {
            let mut val = |name: &str| it.next().ok_or_else(|| anyhow!("{name} needs a value"));
            match a.as_str() {
                "--profile" => o.profile = Some(val("--profile")?.into()),
                "--storage-state" => o.storage_state = Some(val("--storage-state")?.into()),
                "--grant-origin" => o.grant_origins.push(val("--grant-origin")?),
                "--human-cookies" => o.human_cookies = Some(val("--human-cookies")?.into()),
                "--isolated" => {}
                other => bail!("unknown argument {other}"),
            }
        }
        if o.profile.is_some() && o.storage_state.is_some() {
            bail!(
                "--storage-state seeds an ephemeral session; it cannot be combined with --profile"
            );
        }
        Ok(o)
    }
}

/// Holds the profile lock for the life of the process.
pub struct Held(#[allow(dead_code)] Option<File>);

/// Claim a profile directory: exactly one process at a time.
pub fn lock_profile(dir: &std::path::Path) -> Result<File> {
    std::fs::create_dir_all(dir).with_context(|| format!("create profile {}", dir.display()))?;
    let f = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(".manuk-profile.lock"))?;
    f.try_lock().map_err(|_| {
        anyhow!(
            "profile {} is already in use by another manuk-mcp process",
            dir.display()
        )
    })?;
    Ok(f)
}

/// Configure the engine's process-global state. Must run before any page loads (and before the
/// session thread starts).
pub fn apply(o: &Options) -> Result<Held> {
    let human = o
        .human_cookies
        .clone()
        .unwrap_or_else(manuk_net::cookie_store_path);
    let held = match &o.profile {
        Some(dir) => {
            let f = lock_profile(dir)?;
            std::env::set_var("MANUK_STATE", dir);
            Some(f)
        }
        None => {
            manuk_net::set_ephemeral(true);
            None
        }
    };
    for origin in &o.grant_origins {
        let n = manuk_net::storage_state::grant_from_profile(&human, origin)?;
        eprintln!(
            "manuk-mcp: grant: {n} cookie(s) for {origin} copied from {}",
            human.display()
        );
    }
    if let Some(p) = &o.storage_state {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(p)?)
            .with_context(|| format!("parse storage state {}", p.display()))?;
        let (c, l) = manuk_net::storage_state::import(&v)?;
        eprintln!(
            "manuk-mcp: storage state: {c} cookie(s), {l} localStorage key(s) from {}",
            p.display()
        );
    }
    Ok(Held(held))
}
