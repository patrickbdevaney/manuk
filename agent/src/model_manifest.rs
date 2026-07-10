//! INFERENCE.MD §2 — the **local model manifest**.
//!
//! Four selectable entries. The manifest is a **menu of options, not a download list**:
//! a setup run fetches exactly one entry, never all four.
//!
//! Every field below was verified against the HuggingFace API on **2026-07-10** (repo
//! exists, quant filename exists, byte size read from the blob listing). Nothing here is
//! guessed; if an entry ever 404s, the manifest is wrong and should be fixed, not papered
//! over.
//!
//! **Quant choice.** Where a repo ships both a naive `Q4_0` artifact and a higher-fidelity
//! *dynamic* requant, we take the dynamic one — for Gemma-4-E4B that means Unsloth's
//! `UD-Q4_K_XL` rather than Google's `q4_0`. The `source` field records which, so the log
//! says what was actually downloaded.
//!
//! **Excluded on purpose.** Ornith (coding-agent specialized: Terminal-Bench/SWE-Bench-
//! tuned, the wrong task shape) is not in the manifest, and **no "Heretic"/decensored
//! variant is ever eligible** — HF surfaces several for these base models
//! (`…-heretic-GGUF`, `…-HERETIC-UNCENSORED`) and none may be selected.
//!
//! **Verification status is part of the data.** Only two entries were run against the
//! capability suite; the other two are `Verified::AvailableUnverified` and must be reported
//! that way. Being present and functional is not the same as being validated.

/// Whether an entry has been run against the agent capability suite on this harness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verified {
    /// Run against the four-capability suite; see `CLAUDE.md` for the pass rate.
    OnCapabilitySuite,
    /// Downloadable and selectable, but **not** run against the suite here. Must never be
    /// implied to be validated merely because it is present.
    AvailableUnverified,
}

/// One selectable local model. (No `Eq`: `download_gb` is an `f32`.)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelEntry {
    /// Stable key used by `--model <key>`.
    pub key: &'static str,
    /// Human-readable model name.
    pub name: &'static str,
    /// HuggingFace repo holding the GGUF.
    pub repo: &'static str,
    /// Exact GGUF filename within the repo.
    pub gguf: &'static str,
    /// Multimodal projector — these models have native vision, which the agent's
    /// screenshot channel needs.
    pub mmproj: &'static str,
    /// Approximate download size, GGUF + mmproj, in GB (from the HF blob listing).
    pub download_gb: f32,
    /// Minimum system RAM to run comfortably, in GB.
    pub min_ram_gb: u32,
    /// Where the quant came from, recorded so the log is honest about fidelity.
    pub source: &'static str,
    /// Extra `llama-server` flags this entry needs, beyond the common ones.
    pub server_flags: &'static [&'static str],
    pub verified: Verified,
}

/// The manifest. Order is the autodetect preference order, most capable first.
pub const MANIFEST: &[ModelEntry] = &[
    ModelEntry {
        key: "qwen3.5-9b-q4km",
        name: "Qwen3.5-9B (Q4_K_M)",
        repo: "unsloth/Qwen3.5-9B-GGUF",
        gguf: "Qwen3.5-9B-Q4_K_M.gguf",
        mmproj: "mmproj-F16.gguf",
        download_gb: 6.60,
        min_ram_gb: 16,
        source: "unsloth Q4_K_M (UD-Q4_K_XL also available, larger)",
        server_flags: &[],
        verified: Verified::AvailableUnverified,
    },
    ModelEntry {
        key: "gemma-4-e4b",
        name: "Gemma-4-E4B (QAT, dynamic requant)",
        repo: "unsloth/gemma-4-E4B-it-qat-GGUF",
        gguf: "gemma-4-E4B-it-qat-UD-Q4_K_XL.gguf",
        mmproj: "mmproj-F16.gguf",
        download_gb: 5.21,
        min_ram_gb: 12,
        // The directive's preference, applied: dynamic requant over Google's naive q4_0.
        source: "unsloth UD-Q4_K_XL (preferred over google/…-qat-q4_0-gguf)",
        server_flags: &[],
        verified: Verified::OnCapabilitySuite, // 4/4 measured on this harness — see CLAUDE.md
    },
    ModelEntry {
        key: "qwen3.5-4b-q4km",
        name: "Qwen3.5-4B (Q4_K_M)",
        repo: "unsloth/Qwen3.5-4B-GGUF",
        gguf: "Qwen3.5-4B-Q4_K_M.gguf",
        mmproj: "mmproj-F16.gguf",
        download_gb: 3.41,
        min_ram_gb: 8,
        source: "unsloth Q4_K_M (UD-Q4_K_XL also available, larger)",
        server_flags: &[],
        verified: Verified::OnCapabilitySuite,
    },
    ModelEntry {
        key: "gemma-4-e2b-mobile",
        name: "Gemma-4-E2B mobile (QAT)",
        repo: "unsloth/gemma-4-E2B-it-qat-mobile-GGUF",
        // This repo ships no Q4-class dynamic requant; UD-Q2_K_XL is the only XL quant.
        gguf: "gemma-4-E2B-it-qat-UD-Q2_K_XL.gguf",
        mmproj: "mmproj-F16.gguf",
        download_gb: 3.18,
        min_ram_gb: 6,
        source: "unsloth UD-Q2_K_XL (repo ships no Q4-class dynamic requant)",
        server_flags: &[],
        verified: Verified::AvailableUnverified,
    },
];

/// The default when the user expresses no preference.
pub const DEFAULT_KEY: &str = "gemma-4-e4b";

/// Look up an entry by key.
pub fn by_key(key: &str) -> Option<&'static ModelEntry> {
    MANIFEST.iter().find(|m| m.key == key)
}

/// Pick an entry for a machine with `ram_gb` of system RAM.
///
/// Deliberately conservative and **deterministic**: the most capable entry whose
/// `min_ram_gb` fits, but never above the documented default unless the machine is clearly
/// large. A user who cares picks explicitly with `--model`.
pub fn autodetect(ram_gb: u32) -> &'static ModelEntry {
    // The directive's rule: `gemma-4-e4b` if capable, `gemma-4-e2b-mobile` if constrained.
    // We do not silently promote to the 9B — that is an explicit choice.
    let default = by_key(DEFAULT_KEY).expect("the default key is in the manifest");
    if ram_gb >= default.min_ram_gb {
        default
    } else {
        by_key("gemma-4-e2b-mobile").expect("the fallback key is in the manifest")
    }
}

/// A model file's canonical HuggingFace download URL.
pub fn download_url(entry: &ModelEntry, file: &str) -> String {
    format!("https://huggingface.co/{}/resolve/main/{}", entry.repo, file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_the_default_exists() {
        let mut keys: Vec<&str> = MANIFEST.iter().map(|m| m.key).collect();
        let n = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), n, "manifest keys must be unique");
        assert!(by_key(DEFAULT_KEY).is_some());
        assert_eq!(MANIFEST.len(), 4, "the directive specifies four entries");
    }

    /// Autodetect follows the directive: the default when capable, the mobile entry when
    /// constrained — and it never silently promotes to the 9B.
    #[test]
    fn autodetect_picks_the_default_when_capable_and_the_mobile_entry_when_not() {
        assert_eq!(autodetect(64).key, "gemma-4-e4b");
        assert_eq!(autodetect(32).key, "gemma-4-e4b");
        assert_eq!(autodetect(12).key, "gemma-4-e4b");
        assert_eq!(autodetect(11).key, "gemma-4-e2b-mobile");
        assert_eq!(autodetect(4).key, "gemma-4-e2b-mobile");

        // A huge machine still gets the default, not the 9B: that is an explicit choice.
        assert_ne!(autodetect(512).key, "qwen3.5-9b-q4km");
    }

    /// Verification status is data, not vibes: exactly the two entries the directive's
    /// acceptance gate names are marked as suite-verified. If a run is not performed, the
    /// entry must be `AvailableUnverified` — presence is not validation.
    #[test]
    fn only_the_two_gated_entries_are_marked_suite_verified() {
        let verified: Vec<&str> = MANIFEST
            .iter()
            .filter(|m| m.verified == Verified::OnCapabilitySuite)
            .map(|m| m.key)
            .collect();
        assert_eq!(verified, vec!["gemma-4-e4b", "qwen3.5-4b-q4km"]);

        let unverified: Vec<&str> = MANIFEST
            .iter()
            .filter(|m| m.verified == Verified::AvailableUnverified)
            .map(|m| m.key)
            .collect();
        assert_eq!(unverified, vec!["qwen3.5-9b-q4km", "gemma-4-e2b-mobile"]);
    }

    /// No decensored / "Heretic" variant may ever enter the manifest, and Ornith is out.
    #[test]
    fn no_decensored_or_excluded_variants() {
        for m in MANIFEST {
            let repo = m.repo.to_lowercase();
            for banned in ["heretic", "uncensored", "decensored", "abliterated", "ornith"] {
                assert!(!repo.contains(banned), "{} is banned in {}", banned, m.repo);
            }
            // Only the two vetted publishers.
            assert!(
                m.repo.starts_with("unsloth/") || m.repo.starts_with("google/") || m.repo.starts_with("Qwen/"),
                "unvetted publisher: {}",
                m.repo
            );
        }
    }

    /// Every entry carries a vision projector: the agent's screenshot channel needs it.
    #[test]
    fn every_entry_has_a_vision_projector_and_a_real_quant() {
        for m in MANIFEST {
            assert!(m.mmproj.ends_with(".gguf"), "{}", m.key);
            assert!(m.gguf.ends_with(".gguf"), "{}", m.key);
            assert!(m.download_gb > 0.5, "{} has an implausible size", m.key);
            assert!(m.min_ram_gb >= 4, "{}", m.key);
            assert!(!m.source.is_empty(), "{} must record its quant source", m.key);
        }
    }

    #[test]
    fn download_urls_are_the_canonical_hf_resolve_form() {
        let e = by_key("qwen3.5-4b-q4km").unwrap();
        assert_eq!(
            download_url(e, e.gguf),
            "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/Qwen3.5-4B-Q4_K_M.gguf"
        );
    }
}
