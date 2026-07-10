//! INFERENCE.MD §5 — **tab session persistence, named collections, and agent-driven tab
//! control.**
//!
//! Three capabilities over the shell's [`crate::tab::Browser`], sharing one on-disk store
//! kept **outside the git repo** (the same convention as §2's model cache):
//!
//! - **Session restore.** The current tab set (URL, order, title, pinned) is persisted, and
//!   on startup reopened *hibernated* — [`restore_into`] calls `Browser::open_restored`,
//!   which creates each tab `Discarded` with no `Page` and no fetch. Only the previously
//!   focused tab is marked for eager load; reopening 40 tabs is 40 URLs of metadata, not 40
//!   page loads. This is load-bearing: eager re-fetch would undo the hibernation-by-default
//!   memory model the whole project rests on.
//! - **Named collections.** An explicit "save these tabs as `<name>`", stored under a
//!   distinct key from the auto-restored session. Multiple coexist; saving one never touches
//!   another or the session (the collections file is a name→tabs map, read-modify-written).
//! - **Agent-driven tab control.** [`TabSelector`] (close a *set* by domain / title / index,
//!   not just one index), [`open_from_saved`] (open a tab from persisted history — depends on
//!   the store existing, per the sequencing constraint), and [`open_search`] (a convenience
//!   wrapper over navigate using the **configurable** search template, defaulting to Google).
//!   These are the concrete capabilities; exposing them in the agent's JSON action schema is
//!   Axis H's H3 surface and follows it, not precedes it.
//!
//! ## Sensitivity of stored URLs (the directive asks us to flag, not assume)
//!
//! URLs are mostly low-stakes, but not always: a URL can carry credentials in its userinfo
//! (`https://user:pass@host`) or a bearer/session token in a query parameter
//! (`?access_token=…`). Persisting those verbatim would write a secret to a plaintext file.
//! So the store **redacts** those specific fields on save ([`redact_for_storage`]) and
//! records that it did, rather than treating every URL as equally safe. The rest of the URL
//! (host, path) is preserved so restore still lands the user in the right place to re-auth.

// The §5 store + tab-control operations are a shell capability wired to the GUI chrome and
// the agent action loop as follow-up; today they are driven by the unit tests below (the
// same convention as `tab.rs`/`panel.rs`).
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::chrome::{self, Settings};
use crate::tab::Browser;

/// One persisted tab. Order is the position in the surrounding `Vec`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabRecord {
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub pinned: bool,
}

impl TabRecord {
    pub fn new(url: impl Into<String>, title: impl Into<String>, pinned: bool) -> Self {
        TabRecord {
            url: url.into(),
            title: title.into(),
            pinned,
        }
    }
}

/// A persisted session: the ordered tab set plus which one was focused.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub tabs: Vec<TabRecord>,
    /// Index into `tabs` of the focused tab — the only one restored eagerly. `None` means no
    /// tab loads eagerly (all wake on focus).
    #[serde(default)]
    pub focused: Option<usize>,
}

/// The on-disk store. One directory, distinct files: `session.json` and `collections.json`.
pub struct SessionStore {
    dir: PathBuf,
}

impl SessionStore {
    /// Resolve the state directory outside the repo, in XDG order:
    /// `$MANUK_STATE` → `$XDG_STATE_HOME/manuk` → `$HOME/.local/state/manuk`. State (not
    /// cache): a discarded session should survive a cache wipe. The directory is created if
    /// missing.
    pub fn open() -> Result<Self> {
        let dir = if let Some(d) = std::env::var_os("MANUK_STATE") {
            PathBuf::from(d)
        } else if let Some(d) = std::env::var_os("XDG_STATE_HOME") {
            PathBuf::from(d).join("manuk")
        } else if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".local/state/manuk")
        } else {
            anyhow::bail!("cannot resolve a state directory: none of MANUK_STATE, XDG_STATE_HOME, HOME are set");
        };
        Ok(SessionStore::with_dir(dir))
    }

    /// Use an explicit directory (tests point this at a tempdir).
    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        SessionStore { dir: dir.into() }
    }

    fn session_path(&self) -> PathBuf {
        self.dir.join("session.json")
    }

    fn collections_path(&self) -> PathBuf {
        self.dir.join("collections.json")
    }

    fn ensure_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("creating state dir {}", self.dir.display()))
    }

    // -- auto-restored session -------------------------------------------------

    /// Persist the current session (URLs redacted of embedded secrets). Overwrites the
    /// previous session; does not touch any collection.
    pub fn save_session(&self, session: &Session) -> Result<()> {
        self.ensure_dir()?;
        let safe = Session {
            tabs: session.tabs.iter().map(redact_record).collect(),
            focused: session.focused,
        };
        write_json(&self.session_path(), &safe)
    }

    /// Load the last session, or `None` if nothing was saved.
    pub fn load_session(&self) -> Result<Option<Session>> {
        read_json(&self.session_path())
    }

    // -- named collections -----------------------------------------------------

    /// Save `tabs` under `name`. Preserves every other collection (read-modify-write of the
    /// single map) and never touches the auto-restored session.
    pub fn save_collection(&self, name: &str, tabs: &[TabRecord]) -> Result<()> {
        self.ensure_dir()?;
        let mut all = self.load_collections_map()?;
        all.insert(
            name.to_string(),
            tabs.iter().map(redact_record).collect(),
        );
        write_json(&self.collections_path(), &all)
    }

    /// Load a named collection, or `None` if there is no such name.
    pub fn load_collection(&self, name: &str) -> Result<Option<Vec<TabRecord>>> {
        Ok(self.load_collections_map()?.remove(name))
    }

    /// The names of all saved collections, sorted.
    pub fn list_collections(&self) -> Result<Vec<String>> {
        Ok(self.load_collections_map()?.into_keys().collect())
    }

    /// Delete a named collection. Returns whether it existed.
    pub fn delete_collection(&self, name: &str) -> Result<bool> {
        let mut all = self.load_collections_map()?;
        let existed = all.remove(name).is_some();
        if existed {
            self.ensure_dir()?;
            write_json(&self.collections_path(), &all)?;
        }
        Ok(existed)
    }

    fn load_collections_map(&self) -> Result<BTreeMap<String, Vec<TabRecord>>> {
        Ok(read_json(&self.collections_path())?.unwrap_or_default())
    }
}

/// Export the browser's current tabs as a [`Session`] (order preserved, focused index set).
pub fn session_of(browser: &Browser) -> Session {
    let tabs: Vec<TabRecord> = browser
        .tabs()
        .iter()
        .map(|t| TabRecord::new(t.url.clone(), t.title.clone(), t.is_pinned()))
        .collect();
    let focused = browser
        .active()
        .and_then(|id| browser.tabs().iter().position(|t| t.id == id));
    Session { tabs, focused }
}

/// **Restore a session into `browser`, hibernated.** Every tab is opened `Discarded` (no
/// fetch); the focused tab is returned so the caller can eagerly load *only* it. Returns the
/// focused tab's id, if any.
pub fn restore_into(browser: &mut Browser, session: &Session) -> Option<manuk_compositor::TabId> {
    let mut ids = Vec::with_capacity(session.tabs.len());
    for rec in &session.tabs {
        ids.push(browser.open_restored(rec.url.clone(), rec.title.clone(), rec.pinned));
    }
    let focused = session.focused.and_then(|i| ids.get(i).copied());
    if let Some(id) = focused {
        browser.focus(id);
    }
    focused
}

// ---------------------------------------------------------------------------
// Agent-driven tab control (H3 surface — the operations; JSON-schema exposure follows H3)
// ---------------------------------------------------------------------------

/// How to select a *set* of tabs to close — the directive's "needs a selector, not just an
/// index".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TabSelector {
    /// All tabs whose URL host equals this (case-insensitive, `www.` ignored).
    Domain(String),
    /// All tabs whose title contains this substring (case-insensitive).
    TitleContains(String),
    /// An explicit list of tab indices (positions in the current order).
    Indices(Vec<usize>),
}

impl TabSelector {
    /// The ids in `browser` this selector matches, in current tab order.
    pub fn matches(&self, browser: &Browser) -> Vec<manuk_compositor::TabId> {
        let tabs = browser.tabs();
        match self {
            TabSelector::Domain(d) => {
                let want = normalize_host(d);
                tabs.iter()
                    .filter(|t| host_of(&t.url).map(|h| normalize_host(&h) == want).unwrap_or(false))
                    .map(|t| t.id)
                    .collect()
            }
            TabSelector::TitleContains(s) => {
                let needle = s.to_ascii_lowercase();
                tabs.iter()
                    .filter(|t| t.title.to_ascii_lowercase().contains(&needle))
                    .map(|t| t.id)
                    .collect()
            }
            TabSelector::Indices(ix) => ix
                .iter()
                .filter_map(|&i| tabs.get(i).map(|t| t.id))
                .collect(),
        }
    }
}

/// Close every tab matching `selector`. Returns how many were closed.
pub fn close_matching(browser: &mut Browser, selector: &TabSelector) -> usize {
    let ids = selector.matches(browser);
    let n = ids.len();
    for id in ids {
        browser.close(id);
    }
    n
}

/// **Open a tab from persisted history.** Depends on the store existing (the sequencing
/// constraint): the agent may only open a URL that is actually in a saved session or
/// collection, so it can't be steered into opening an arbitrary attacker URL through this
/// action. Opens hibernated (not focused); returns the new tab id, or `None` if `url` is not
/// in `known`.
pub fn open_from_saved(
    browser: &mut Browser,
    known: &[TabRecord],
    url: &str,
) -> Option<manuk_compositor::TabId> {
    let rec = known.iter().find(|r| r.url == url)?;
    Some(browser.open_restored(rec.url.clone(), rec.title.clone(), rec.pinned))
}

/// **Open a tab with a search query.** A convenience wrapper over navigate using the
/// configurable search template (default Google via [`chrome::GOOGLE_SEARCH_TEMPLATE`]), not
/// a hardcoded provider. Opens hibernated; returns the new tab id.
pub fn open_search(browser: &mut Browser, query: &str, settings: &Settings) -> manuk_compositor::TabId {
    let url = chrome::search_url(query, settings);
    browser.open_restored(url, format!("Search: {query}"), false)
}

/// [`Settings`] with the directive's default search engine (Google), for callers that want it
/// without hand-writing the template.
pub fn default_search_settings() -> Settings {
    Settings {
        search_template: chrome::GOOGLE_SEARCH_TEMPLATE.to_string(),
        ..Settings::default()
    }
}

// ---------------------------------------------------------------------------
// URL sensitivity — redact secrets before writing them to a plaintext store
// ---------------------------------------------------------------------------

/// Query-parameter names that commonly carry a bearer/session secret.
const SECRET_PARAMS: &[&str] = &[
    "access_token",
    "id_token",
    "token",
    "auth",
    "session",
    "sid",
    "sessionid",
    "api_key",
    "apikey",
    "code",
    "password",
];

/// Whether this URL carries something that should not be written verbatim to disk. Returns a
/// human reason if so, for logging/flagging.
pub fn flag_sensitive(url: &str) -> Option<&'static str> {
    let Ok(u) = url::Url::parse(url) else {
        return None;
    };
    if !u.username().is_empty() || u.password().is_some() {
        return Some("embedded credentials in userinfo");
    }
    for (k, _) in u.query_pairs() {
        if SECRET_PARAMS.contains(&k.to_ascii_lowercase().as_str()) {
            return Some("secret-bearing query parameter");
        }
    }
    None
}

/// Redact a URL for storage: strip userinfo and replace secret query-parameter values with
/// `REDACTED`, preserving host/path/other params so restore still lands the right place.
pub fn redact_for_storage(url: &str) -> String {
    let Ok(mut u) = url::Url::parse(url) else {
        return url.to_string();
    };
    let touched_userinfo = !u.username().is_empty() || u.password().is_some();
    if touched_userinfo {
        let _ = u.set_username("");
        let _ = u.set_password(None);
    }
    // Rebuild the query with secret values redacted.
    let redacted: Vec<(String, String)> = u
        .query_pairs()
        .map(|(k, v)| {
            if SECRET_PARAMS.contains(&k.to_ascii_lowercase().as_str()) {
                (k.into_owned(), "REDACTED".to_string())
            } else {
                (k.into_owned(), v.into_owned())
            }
        })
        .collect();
    if u.query().is_some() {
        if redacted.iter().all(|(_, v)| v != "REDACTED") && !touched_userinfo {
            // Nothing to change.
        } else {
            let mut qs = u.query_pairs_mut();
            qs.clear();
            for (k, v) in &redacted {
                qs.append_pair(k, v);
            }
            drop(qs);
        }
    }
    u.to_string()
}

fn redact_record(r: &TabRecord) -> TabRecord {
    TabRecord {
        url: redact_for_storage(&r.url),
        title: r.title.clone(),
        pinned: r.pinned,
    }
}

// -- host helpers ------------------------------------------------------------

fn host_of(url: &str) -> Option<String> {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string))
}

fn normalize_host(h: &str) -> String {
    h.trim().to_ascii_lowercase().trim_start_matches("www.").to_string()
}

// -- json io -----------------------------------------------------------------

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    std::fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes).with_context(|| {
            format!("parsing {}", path.display())
        })?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use manuk_text::FontContext;

    fn tmpdir(tag: &str) -> PathBuf {
        // Unique-per-test dir under the scratch/temp area; no wall clock (deterministic tag).
        let base = std::env::temp_dir().join("manuk-session-tests");
        let d = base.join(tag);
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    // -- session restore is hibernated ---------------------------------------

    /// The load-bearing property: restoring N tabs opens N hibernated tabs and eagerly loads
    /// **at most one** — no 40-page fetch storm.
    #[test]
    fn restore_reopens_tabs_hibernated_not_eagerly_fetched() {
        let mut browser = Browser::new(8);
        let session = Session {
            tabs: (0..40)
                .map(|i| TabRecord::new(format!("https://site{i}.test/"), format!("Site {i}"), i == 0))
                .collect(),
            focused: Some(3),
        };

        let focused = restore_into(&mut browser, &session);
        assert_eq!(browser.tabs().len(), 40);

        // Not a single tab holds a Page — nothing was fetched or laid out on restore.
        assert!(
            browser.tabs().iter().all(|t| t.page().is_none()),
            "restored tabs must be hibernated (no Page)"
        );
        // The focused index maps to the focused tab, and pinned metadata survived.
        let f = focused.expect("a focused tab");
        assert_eq!(browser.active(), Some(f));
        assert!(browser.tab(browser.tabs()[0].id).unwrap().is_pinned(), "tab 0 was pinned");
        assert!(!browser.tabs()[1].is_pinned());
    }

    /// Focused index `None` restores everything hibernated with nothing eager.
    #[test]
    fn restore_with_no_focus_loads_nothing_eagerly() {
        let mut browser = Browser::new(8);
        let session = Session {
            tabs: vec![TabRecord::new("https://a.test/", "A", false)],
            focused: None,
        };
        assert!(restore_into(&mut browser, &session).is_none());
        assert!(browser.tabs().iter().all(|t| t.page().is_none()));
    }

    // -- persistence round-trips ---------------------------------------------

    #[test]
    fn session_round_trips_through_the_store() {
        let store = SessionStore::with_dir(tmpdir("session-roundtrip"));
        assert!(store.load_session().unwrap().is_none(), "no session yet");

        let session = Session {
            tabs: vec![
                TabRecord::new("https://a.test/", "A", true),
                TabRecord::new("https://b.test/x", "B", false),
            ],
            focused: Some(1),
        };
        store.save_session(&session).unwrap();
        assert_eq!(store.load_session().unwrap().unwrap(), session);
    }

    /// Collections are distinct from the session and from each other: saving one leaves the
    /// others and the session untouched.
    #[test]
    fn collections_are_independent_of_each_other_and_the_session() {
        let store = SessionStore::with_dir(tmpdir("collections-independent"));
        store
            .save_session(&Session {
                tabs: vec![TabRecord::new("https://session.test/", "S", false)],
                focused: Some(0),
            })
            .unwrap();

        let work = vec![TabRecord::new("https://jira.test/", "Jira", false)];
        let read = vec![TabRecord::new("https://news.test/", "News", false)];
        store.save_collection("work", &work).unwrap();
        store.save_collection("reading", &read).unwrap();

        assert_eq!(store.load_collection("work").unwrap().unwrap(), work);
        assert_eq!(store.load_collection("reading").unwrap().unwrap(), read);
        assert_eq!(store.list_collections().unwrap(), vec!["reading".to_string(), "work".to_string()]);

        // Saving/deleting a collection never disturbs the session.
        assert_eq!(store.load_session().unwrap().unwrap().tabs[0].url, "https://session.test/");
        assert!(store.delete_collection("work").unwrap());
        assert!(store.load_collection("work").unwrap().is_none());
        assert_eq!(store.load_collection("reading").unwrap().unwrap(), read); // untouched
        assert!(!store.delete_collection("work").unwrap(), "already gone");
    }

    // -- agent-driven tab control --------------------------------------------

    fn load(browser: &mut Browser, id: manuk_compositor::TabId, url: &str) {
        let fonts = FontContext::new();
        let page = manuk_page::Page::load("<title>x</title><body>x</body>", url, &fonts, 800.0);
        browser.load(id, page, "<body>x</body>".into());
    }

    #[test]
    fn close_by_domain_closes_the_whole_set() {
        let mut b = Browser::new(8);
        let a1 = b.open("https://ads.test/1");
        load(&mut b, a1, "https://ads.test/1");
        let a2 = b.open("https://www.ads.test/2"); // www. must match ads.test
        load(&mut b, a2, "https://www.ads.test/2");
        let keep = b.open("https://keep.test/");
        load(&mut b, keep, "https://keep.test/");

        let closed = close_matching(&mut b, &TabSelector::Domain("ads.test".into()));
        assert_eq!(closed, 2);
        assert_eq!(b.tabs().len(), 1);
        assert_eq!(b.tabs()[0].url, "https://keep.test/");
    }

    #[test]
    fn close_by_title_and_by_index() {
        let mut b = Browser::new(8);
        let t0 = b.open("https://a.test/");
        load(&mut b, t0, "https://a.test/");
        b.set_loaded(t0, "https://a.test/".into(), "Invoice 2024".into(), 0.0);
        let t1 = b.open("https://b.test/");
        load(&mut b, t1, "https://b.test/");
        b.set_loaded(t1, "https://b.test/".into(), "Dashboard".into(), 0.0);

        assert_eq!(close_matching(&mut b, &TabSelector::TitleContains("invoice".into())), 1);
        assert_eq!(b.tabs().len(), 1);
        assert_eq!(close_matching(&mut b, &TabSelector::Indices(vec![0])), 1);
        assert!(b.tabs().is_empty());
    }

    /// Open-from-history only opens URLs actually present in the saved set — it cannot be
    /// steered into an arbitrary URL.
    #[test]
    fn open_from_saved_refuses_unknown_urls() {
        let mut b = Browser::new(8);
        let known = vec![TabRecord::new("https://known.test/page", "Known", false)];

        assert!(open_from_saved(&mut b, &known, "https://evil.test/").is_none());
        assert!(b.tabs().is_empty(), "an unknown url opens nothing");

        let id = open_from_saved(&mut b, &known, "https://known.test/page").unwrap();
        assert_eq!(b.tabs().len(), 1);
        assert!(b.tab(id).unwrap().page().is_none(), "opened hibernated");
    }

    #[test]
    fn open_search_uses_the_configurable_template() {
        let mut b = Browser::new(8);
        let id = open_search(&mut b, "rust browser engine", &default_search_settings());
        let url = &b.tab(id).unwrap().url;
        assert!(url.starts_with("https://www.google.com/search?q="), "{url}");
        assert!(url.contains("rust"));

        // A different configured engine is honored, not overridden.
        let ddg = Settings::default(); // duckduckgo
        let id2 = open_search(&mut b, "x", &ddg);
        assert!(b.tab(id2).unwrap().url.contains("duckduckgo.com"));
    }

    // -- URL sensitivity ------------------------------------------------------

    #[test]
    fn credential_bearing_urls_are_flagged_and_redacted_on_save() {
        assert!(flag_sensitive("https://user:pass@host.test/").is_some());
        assert!(flag_sensitive("https://host.test/cb?access_token=secret123&page=2").is_some());
        assert!(flag_sensitive("https://host.test/normal?page=2").is_none());

        // userinfo is stripped…
        let r = redact_for_storage("https://user:pass@host.test/path");
        assert!(!r.contains("user"), "userinfo removed: {r}");
        assert!(!r.contains("pass"), "password removed: {r}");
        assert!(r.contains("host.test"));

        // …and a token value is replaced, but the benign param and path survive.
        let r2 = redact_for_storage("https://host.test/cb?access_token=secret123&page=2");
        assert!(!r2.contains("secret123"), "token value redacted: {r2}");
        assert!(r2.contains("REDACTED"));
        assert!(r2.contains("page=2"), "benign param kept: {r2}");
    }

    /// The store redacts on save, so a secret never reaches disk even if the caller passes
    /// a raw session in.
    #[test]
    fn the_store_redacts_secrets_before_writing() {
        let dir = tmpdir("redact-on-save");
        let store = SessionStore::with_dir(&dir);
        store
            .save_session(&Session {
                tabs: vec![TabRecord::new("https://u:p@host.test/x?token=abc", "T", false)],
                focused: Some(0),
            })
            .unwrap();
        let raw = std::fs::read_to_string(dir.join("session.json")).unwrap();
        assert!(!raw.contains("abc"), "token must not be on disk: {raw}");
        assert!(!raw.contains(":p@") && !raw.contains("u:p"), "creds must not be on disk: {raw}");
    }
}
