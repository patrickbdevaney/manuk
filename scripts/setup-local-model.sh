#!/usr/bin/env bash
# INFERENCE.MD §2 — bundle a local model, with zero decisions required.
#
#   ./scripts/setup-local-model.sh                  # autodetect, download ONE model, launch
#   ./scripts/setup-local-model.sh --model qwen3.5-4b-q4km
#   ./scripts/setup-local-model.sh --list          # show the manifest, download nothing
#   ./scripts/setup-local-model.sh --no-launch     # fetch only
#
# Guarantees, each of which the directive calls out explicitly:
#   * Exactly ONE model is downloaded per run. The manifest is a menu, not a download list.
#   * An already-present file is reused, never re-fetched (resumable via curl -C -).
#   * Nothing is ever written inside the git repo. Everything lives in $MANUK_CACHE.
#   * No prompts, no menu: with no flags it detects, picks, downloads, and launches.
#   * The llama.cpp runtime binary is fetched once and shared across model choices.
set -euo pipefail

MANUK_CACHE="${MANUK_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/manuk}"
MODELS_DIR="$MANUK_CACHE/models"
BIN_DIR="$MANUK_CACHE/bin"
LOG_DIR="$MANUK_CACHE/log"
PORT="${MANUK_LLAMA_PORT:-8080}"

# ---------------------------------------------------------------------------
# Manifest. Mirrors agent/src/model_manifest.rs; verified against the HF API
# 2026-07-10. Keep the two in sync — the Rust copy is the one the agent reads.
# Fields: key|repo|gguf|mmproj|min_ram_gb|download_gb|source
# ---------------------------------------------------------------------------
MANIFEST=(
"gemma-4-e4b|unsloth/gemma-4-E4B-it-qat-GGUF|gemma-4-E4B-it-qat-UD-Q4_K_XL.gguf|mmproj-F16.gguf|12|5.21|unsloth UD-Q4_K_XL (preferred over google naive q4_0)"
"gemma-4-e2b-mobile|unsloth/gemma-4-E2B-it-qat-mobile-GGUF|gemma-4-E2B-it-qat-UD-Q2_K_XL.gguf|mmproj-F16.gguf|6|3.18|unsloth UD-Q2_K_XL (repo ships no Q4-class dynamic requant)"
"qwen3.5-4b-q4km|unsloth/Qwen3.5-4B-GGUF|Qwen3.5-4B-Q4_K_M.gguf|mmproj-F16.gguf|8|3.41|unsloth Q4_K_M"
"qwen3.5-9b-q4km|unsloth/Qwen3.5-9B-GGUF|Qwen3.5-9B-Q4_K_M.gguf|mmproj-F16.gguf|16|6.60|unsloth Q4_K_M"
)
DEFAULT_KEY="gemma-4-e4b"
CONSTRAINED_KEY="gemma-4-e2b-mobile"

die() { echo "error: $*" >&2; exit 1; }
# Status goes to stderr. `fetch_llama_server` returns the binary path on stdout, so any
# status printed there would be captured as part of the path.
info() { echo "==> $*" >&2; }

entry_for() {
  local key="$1"
  for row in "${MANIFEST[@]}"; do
    [[ "${row%%|*}" == "$key" ]] && { echo "$row"; return 0; }
  done
  return 1
}

list_manifest() {
  # `local` matters: without it the loop clobbers the caller's $key and the error
  # message names the wrong model.
  local row k _repo _gguf _mm ram size src
  printf '%-20s %-10s %-9s %s\n' KEY RAM_MIN SIZE SOURCE
  for row in "${MANIFEST[@]}"; do
    IFS='|' read -r k _repo _gguf _mm ram size src <<<"$row"
    printf '%-20s %-10s %-9s %s\n' "$k" "${ram}GB" "${size}GB" "$src"
  done
  echo
  echo "default: $DEFAULT_KEY   (constrained fallback: $CONSTRAINED_KEY)"
  echo "verified on the capability suite: see CLAUDE.md — presence here is NOT validation."
}

detect_ram_gb() {
  if [[ -r /proc/meminfo ]]; then
    awk '/MemTotal/ {printf "%d", $2/1024/1024}' /proc/meminfo
  elif command -v sysctl >/dev/null 2>&1; then
    echo $(( $(sysctl -n hw.memsize) / 1024 / 1024 / 1024 ))   # macOS
  else
    echo 8
  fi
}

# CUDA / Metal / Vulkan / CPU — picks the llama.cpp release asset flavor.
detect_accel() {
  case "$(uname -s)" in
    Darwin) echo "macos" ; return ;;
  esac
  if [[ -e /dev/nvidia0 ]] || command -v nvidia-smi >/dev/null 2>&1; then
    echo "cuda"; return
  fi
  if command -v vulkaninfo >/dev/null 2>&1 || [[ -e /dev/dri ]]; then
    echo "vulkan"; return
  fi
  echo "cpu"
}

asset_for_target() {
  local accel="$1" arch; arch="$(uname -m)"
  case "$(uname -s)" in
    Darwin) echo "macos-$( [[ $arch == arm64 ]] && echo arm64 || echo x64 )"; return ;;
  esac
  case "$arch" in
    x86_64|amd64) arch=x64 ;;
    aarch64|arm64) arch=arm64 ;;
    *) die "unsupported arch: $arch" ;;
  esac
  # CUDA release assets are not published for every tag; Vulkan runs on NVIDIA too and is
  # always present. Prefer vulkan over a from-source CUDA build.
  case "$accel" in
    cuda|vulkan) echo "ubuntu-vulkan-$arch" ;;
    *)           echo "ubuntu-$arch" ;;
  esac
}

fetch_llama_server() {
  local server="$BIN_DIR/llama-server"
  if [[ -x "$server" ]]; then
    info "llama-server already present: $server"
    echo "$server"; return
  fi
  mkdir -p "$BIN_DIR"

  local accel asset tag url tmp
  accel="$(detect_accel)"
  asset="$(asset_for_target "$accel")"
  info "acceleration: $accel   asset: $asset"

  tag="$(curl -fsSL https://api.github.com/repos/ggml-org/llama.cpp/releases/latest \
        | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)"
  [[ -n "$tag" ]] || die "could not resolve the latest llama.cpp release tag"
  url="https://github.com/ggml-org/llama.cpp/releases/download/${tag}/llama-${tag}-bin-${asset}.tar.gz"

  info "downloading llama.cpp $tag ($asset)"
  tmp="$(mktemp -d)"
  if ! curl -fL --retry 3 -o "$tmp/llama.tar.gz" "$url"; then
    rm -rf "$tmp"
    # Building from source is the documented fallback; we do not do it silently.
    die "no prebuilt asset at $url — build llama.cpp from source and put llama-server in $BIN_DIR"
  fi
  tar -xzf "$tmp/llama.tar.gz" -C "$tmp"
  local found; found="$(find "$tmp" -type f -name 'llama-server' -perm -u+x | head -1)"
  [[ -n "$found" ]] || die "llama-server not found inside the release archive"
  # Ship the whole lib dir alongside: the binary links against libggml*.so.
  local libdir; libdir="$(dirname "$found")"
  cp -a "$libdir/." "$BIN_DIR/"
  chmod +x "$BIN_DIR/llama-server"
  rm -rf "$tmp"
  info "installed $BIN_DIR/llama-server"
  echo "$BIN_DIR/llama-server"
}

# Download one file, reusing/resuming what is already on disk.
fetch_file() {
  local repo="$1" file="$2" dest="$3"
  if [[ -s "$dest" ]]; then
    info "reusing cached $(basename "$dest")"
    return
  fi
  mkdir -p "$(dirname "$dest")"
  info "downloading $file from $repo"
  curl -fL --retry 3 -C - -o "$dest" \
    "https://huggingface.co/${repo}/resolve/main/${file}" \
    || die "download failed: $repo/$file"
}

main() {
  local key="" launch=1
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --model) key="${2:-}"; shift 2 ;;
      --list) list_manifest; exit 0 ;;
      --no-launch) launch=0; shift ;;
      --port) PORT="${2:-}"; shift 2 ;;
      -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
      *) die "unknown flag: $1 (try --help)" ;;
    esac
  done

  if [[ -z "$key" ]]; then
    local ram; ram="$(detect_ram_gb)"
    # The directive's rule, verbatim: the default if capable, the mobile entry if not.
    if (( ram >= 12 )); then key="$DEFAULT_KEY"; else key="$CONSTRAINED_KEY"; fi
    info "autodetect: ${ram}GB RAM -> $key  (override with --model <key>; --list to see all)"
  fi

  local row; row="$(entry_for "$key")" || { echo; list_manifest; die "unknown model key: $key"; }
  IFS='|' read -r _k repo gguf mmproj ram size src <<<"$row"

  info "model:  $key"
  info "repo:   $repo"
  info "quant:  $gguf   [$src]"
  info "size:   ~${size}GB (+ vision projector)"
  info "cache:  $MANUK_CACHE   (never inside the git repo)"

  local server; server="$(fetch_llama_server)"

  local model_dir="$MODELS_DIR/$key"
  fetch_file "$repo" "$gguf"   "$model_dir/$gguf"
  fetch_file "$repo" "$mmproj" "$model_dir/$mmproj"

  if (( launch == 0 )); then
    info "fetched only (--no-launch). Model at $model_dir"
    exit 0
  fi

  mkdir -p "$LOG_DIR"
  # GPU offload: MANUK_NGL lets the caller pin a layer count. Left unset, we do NOT pass
  # --n-gpu-layers at all — recent llama.cpp then auto-fits layers to free device memory
  # (its `common_fit_params`). Forcing `-ngl 999` *disables* that auto-fit and hard-aborts
  # on any card too small to hold the whole model + vision projector + KV cache; an 8GB GPU
  # holds Qwen-4B but not Gemma-4-E4B at full offload, so 999 is the wrong default.
  local ngl_args=()
  if [[ -n "${MANUK_NGL:-}" ]]; then
    ngl_args=(--n-gpu-layers "$MANUK_NGL")
    info "GPU offload: pinned to $MANUK_NGL layers (MANUK_NGL)"
  else
    info "GPU offload: auto-fit to free device memory (set MANUK_NGL to pin)"
  fi
  info "launching llama-server on 127.0.0.1:$PORT"
  LD_LIBRARY_PATH="$BIN_DIR:${LD_LIBRARY_PATH:-}" \
  nohup "$server" \
      --model "$model_dir/$gguf" \
      --mmproj "$model_dir/$mmproj" \
      --host 127.0.0.1 --port "$PORT" \
      --ctx-size 8192 --jinja "${ngl_args[@]}" \
      >"$LOG_DIR/llama-server.log" 2>&1 &
  local pid=$!
  echo "$pid" > "$MANUK_CACHE/llama-server.pid"

  # Wait for readiness rather than sleeping a guessed interval.
  for _ in $(seq 1 120); do
    if curl -fsS "http://127.0.0.1:$PORT/v1/models" >/dev/null 2>&1; then
      info "ready: http://127.0.0.1:$PORT  (pid $pid, log $LOG_DIR/llama-server.log)"
      echo
      echo "The agent picks this up as a keyless OpenAiCompatBackend:"
      echo "    manuk_agent::local::OpenAiCompatBackend::local_llama($PORT)"
      exit 0
    fi
    kill -0 "$pid" 2>/dev/null || { tail -20 "$LOG_DIR/llama-server.log"; die "llama-server exited"; }
    sleep 1
  done
  die "llama-server did not become ready; see $LOG_DIR/llama-server.log"
}

main "$@"
