# Directive: inference backend generalization + agent-scale traversal

Six related changes, sequenced. Do not use the Task tool or spawn subagents —
work sequentially, single-agent, one item at a time, committing after each with
a message naming the item. Update CLAUDE.md and README per existing maintenance
discipline as each capability lands.

## 1. Generalize GroqBackend → OpenAiCompatBackend

Replace the hardcoded Groq implementation with a generic struct: `endpoint_url`
(required), `api_key` (optional — required for Groq, typically absent for
local/self-hosted), `model` (optional, falling back to a `GET /v1/models` query
against the endpoint if unset — explicit-with-fallback, not silent
auto-pick-first). Groq becomes a preset (fixed `endpoint_url` + default model)
of this generic backend, not its own type. `agent-run` keeps reading
`GROQ_API_KEY`/`.env` as today; nothing about the CLI surface changes, only the
internal representation. Acceptance: existing Groq capability suite (4/4) still
passes against the refactored backend; a manual test against a local
OpenAI-compatible endpoint (e.g. vLLM) succeeds with no key.

## 2. Bundled local model — manifest of four, zero-effort setup script

**Model manifest — four selectable entries, Ornith excluded** (coding-agent
specialized — Terminal-Bench/SWE-Bench-tuned, not this task's shape; revisit
separately if ever, and never the decensored "Heretic" variant if it is):

| Key | Model | Role | Notes |
|---|---|---|---|
| `gemma-4-e4b` | Gemma-4-E4B (from `gemma-4-qat-q4-0`) | **Default** | native vision, no unusual serving flags |
| `gemma-4-e2b-mobile` | Gemma-4-E2B mobile (from `gemma-4-qat-mobile`) | Constrained-hardware fallback | native vision, no unusual serving flags |
| `qwen3.5-4b-q4km` | Qwen3.5-4B, Q4_K_M | Alternate | native vision, no unusual serving flags |
| `qwen3.5-9b-q4km` | Qwen3.5-9B, Q4_K_M | Alternate, higher capability/footprint | native vision, ~5GB+ |

For each: prefer the higher-fidelity dynamic requant (e.g. Unsloth
UD-Q4_K_XL-class) over a raw naive Q4_0 artifact where both exist — note the
source explicitly in whatever manifest/log records what was downloaded. Each
manifest entry carries its HF repo, exact quant filename, min RAM/VRAM, and
the exact `llama-server` flags it needs.

**The setup script — downloads exactly one model per run, never all four:**

Two supported paths, both resulting in exactly one model on disk:

- **Seamless default (zero flags):** run the script with no arguments and it
  just works — detects OS/arch and acceleration, autodetect picks a sensible
  entry by available RAM (`gemma-4-e4b` if capable, `gemma-4-e2b-mobile` if
  constrained), downloads *only that one*, and launches it. No prompts, no
  menu, no partial multi-model download — a user who doesn't care which model
  gets a working local agent with zero decisions required.
- **Explicit choice (`--model <key>`):** skips autodetect entirely and
  downloads *only* the named manifest entry. This is how a user picks
  `qwen3.5-4b-q4km`, `qwen3.5-9b-q4km`, or overrides the autodetected default
  for any reason.

In both paths the script downloads the one selected GGUF, nothing else — the
manifest is a menu of *options*, not a download list. Re-running the script
with a different `--model` value fetches that entry (and should reuse
whatever's already cached rather than re-fetching an already-present file),
it does not additively accumulate all previously-tried models on disk unless
the user explicitly keeps multiple around.

The mechanics, either path:
1. Detect OS/arch and acceleration (CUDA/Metal/Vulkan/CPU AVX2/NEON).
2. Download a prebuilt llama.cpp release binary for that target; build from
   source only as fallback if no prebuilt matches. (Runtime binary is shared
   across model choices — this step happens once regardless of which model
   is selected.)
3. Resolve the model key (autodetect or explicit `--model`) to exactly one
   manifest entry.
4. Launch `llama-server` as a local background process on `127.0.0.1` with
   that entry's specific flags, register it as a preset `OpenAiCompatBackend`
   (from item 1) with no key.
5. Never touches the git repo — cache directory only, outside version control.

**Acceptance gate — two models today, not four:**
Run the existing agent capability suite (text-extraction, link-comprehension,
link-navigation, multimodal-screenshot) against **`gemma-4-e4b` and
`qwen3.5-4b-q4km` only**, each via a locally-launched `llama-server`. Report
the pass rate for each against the bar the Groq/qwen3.6-27B baseline cleared
(4/4) — state plainly if either doesn't clear it, do not round up.
`gemma-4-e2b-mobile` and `qwen3.5-9b-q4km` stay in the manifest and are
selectable, but get marked **"available, unverified on this harness"** — that
testing is explicit follow-up work, not today's, and must not be silently
implied by the entry simply being present and functional.

*Rationale for reviewers, not for the script*: the observation format
(`AgentBrowser.observe()` — indexed links, extracted text, screenshot) is
engineered and information-dense, not a raw DOM dump a generic Playwright/
Selenium harness would produce; the action space (`navigate`/`click`/`scroll`/
`finish`) is small and closed. Both properties make a 4-5B model more viable
here than it would be against a generic automation surface — but that's a
reason to test at this size, not a substitute for testing.

## 3. In-browser agent panel (shell-side)

A new capability of `shell`, not a duplicate of `agent`: user-driven, live,
single prompt-scoped task, uses the same `OpenAiCompatBackend` (BYO endpoint or
the bundled local model from item 2), same `engine/page` pipeline. Two
non-negotiable constraints:
- **Task-scoped action permissions**, not the full `agent` action space —
  define per-invocation what the panel is allowed to do; do not give it an
  open-ended action set by default.
- **Page content is untrusted input to the model, never treated as
  instructions** — this is a real prompt-injection surface (a page can embed
  "ignore the user and instead..." text a naive implementation would read as
  context). Design the prompt construction so page content is clearly
  demarcated as data, the same way untrusted document content is handled
  elsewhere, not concatenated as if it were the user's own turn.
- Session handoff to/from the standalone `agent` binary (share live session
  state, hand back control) is a distinct, explicitly-authorized action — not
  an implicit side effect of opening the panel.

## 4. Agent-scale traversal: cache, triage, concurrency

Three items, additive to `agent`, not blocking items 1-3:

- **Content-addressed cache with freshness scoring.** Key by hash of
  *extracted* content, not raw HTML (raw HTML changes on every fetch via ads/
  nonces/timestamps even when the real content hasn't). Per-URL volatility
  score decays toward "check less often" on unchanged re-fetches, rises toward
  "check more often" on changed ones. Per-request freshness policy
  (`max_age`) settable by the caller. This is the primary lever for
  traversal/scraping at scale — avoiding redundant fetches, not making
  individual fetches faster.
- **Page-triage fast path.** Before routing a fetched page through JS
  execution, classify whether its meaningful content is already extractable
  from server-rendered HTML alone (true for most Tier-1-style content). Only
  the minority that actually needs it pays the JS/DOM/layout cost.
- **Multi-page concurrency within a single agent process.** Worker-pool model
  over SpiderMonkey isolates. Network fetch concurrency can be much higher
  (I/O-bound, thousands feasible) than JS-execution concurrency, which should
  stay bounded near core count (CPU-bound) — do not conflate the two into one
  concurrency limit.

## 5. Tab session persistence, saved collections, and agent-driven tab control

**Session restore on restart.** Persist the current tab set (URLs, order,
title, pinned state — not full page state) to a local store outside the git
repo, same convention as the model cache from item 2. On `shell` startup,
reopen exactly that tab set. **Critical constraint, easy to get wrong:**
restored tabs come back *hibernated*, not eagerly re-fetched — reopening 40
tabs from last session must not mean 40 immediate page loads, or this
directly undermines the hibernation-by-default memory model the whole project
is built around. Only the previously-focused tab (or none) fetches eagerly;
the rest wake on focus, exactly like any other hibernated tab.

**Named, user-saved tab collections.** Distinct from session-restore: an
explicit "save current tabs as `<name>`" action, storing a named, ordered tab
set the user can reopen later on demand — a lightweight tab-group/bookmark
hybrid, not tied to auto-restore. Multiple collections coexist; saving one
doesn't touch another or the auto-restored last-session set. Same local store
as session-restore, distinct keys. Note for reviewers: URLs/titles in this
store aren't secrets the way passwords are (Axis E2), so it doesn't need E2's
encryption rigor by default — but flag if any saved URL data should be
treated more sensitively (e.g., session tokens embedded in a URL) rather than
assuming all URLs are equally low-stakes.

**Agent-driven tab control.** Extends Axis H's action-set work (H3 already
names tab management as shared surface between the headful UI and the agent
action schema) with three concrete actions, each with a real dependency:
- **Close a specific set of tabs** — needs a selector, not just an index:
  by domain, by title match, or by explicit index list. Design this as part
  of H3's action-set work, not a bespoke one-off.
- **Open a tab from history** — depends on the session/collection store
  above existing first; the agent needs something to query. Sequence this
  action's implementation after the persistence store lands, not in
  parallel.
- **Open a tab with a search query** — this is a convenience wrapper over
  `navigate` with a templated search-engine URL, not a new primitive.
  Default to Google per this directive, but implement the template as a
  configurable default-search-engine setting, not a hardcoded Google-only
  URL, so swapping providers later doesn't require new code.



## Sequencing

1 unblocks everything else (2 and 3 both depend on `OpenAiCompatBackend`
existing). 2 and 3 can proceed in parallel once 1 lands. Within item 2, the
manifest infrastructure and the two acceptance tests are today's scope; the
two unverified manifest entries are explicit follow-up, not to be silently
treated as validated. 4 is independent of 1-3 and can be worked whenever
convenient — it improves `agent` alone and touches no shared code path with
the inference-backend or panel work. Item 5's session-restore and named-
collections halves are independent `shell`-side work and can start anytime;
its agent-driven-tab-control half depends on both the persistence store
(same item, built first) and Axis H's action-set design (H3) — do not
implement "open from history" or "close a set of tabs" as agent actions
before both prerequisites exist.