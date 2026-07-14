#!/usr/bin/env bash
# ── THE OOM GUARD. Source this before any cargo build/test.
#
# **The failure this exists to prevent is not slow builds — it is a LIE.**
#
# `ld terminated with signal 9 [Killed]` is the OOM killer, and it looks *exactly* like a compile error:
# cargo returns non-zero, and every wrapper above it reads that as "the code is broken". It has already
# cost this project a false verdict — `falsify.sh` reported **FALSIFIER BROKEN** for two perfectly good
# mutations, and only a retry at `CARGO_BUILD_JOBS=2` proved both (PROCESS #31). An OOM that presents as
# a test result is the worst kind of instrument failure, because it is *believed*.
#
# **Why this box is at risk:** mozjs and Stylo are the two heaviest things in the graph, and LLVM codegen
# peaks around **1.5–2 GB per parallel job**. With 32 logical cores, cargo's default `-j 32` will happily
# ask for ~50 GB of transient RSS on a 31 GB machine. The default is not a setting anyone chose; it is
# `nproc`, and `nproc` knows nothing about LLVM.
#
# So: derive the job count from **available memory**, not from the core count.

# Available RAM in MB (not "free" — `available` is the number that accounts for reclaimable cache).
_mem_avail_mb() { awk '/^MemAvailable:/ {print int($2/1024)}' /proc/meminfo 2>/dev/null || echo 4096; }
_swap_used_pct() {
  awk '/^SwapTotal:/{t=$2} /^SwapFree:/{f=$2} END{ if (t>0) print int((t-f)*100/t); else print 0 }' /proc/meminfo 2>/dev/null || echo 0
}

# ~2 GB per LLVM codegen job, with a 2 GB floor left for the OS and the editor.
_MEM_MB=$(_mem_avail_mb)
_JOBS=$(( (_MEM_MB - 2048) / 2048 ))
[ "$_JOBS" -lt 1 ] && _JOBS=1
_NPROC=$(nproc 2>/dev/null || echo 4)
[ "$_JOBS" -gt "$_NPROC" ] && _JOBS="$_NPROC"

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$_JOBS}"

# A full swap is not fatal on its own — RAM may be plentiful and the swap merely *stale* from an earlier
# spike. But it is worth SAYING, because a machine that is already swapping will thrash under a link, and
# a thrashing link is the one that gets killed. Cycling it (`sudo swapoff -a && sudo swapon -a`) is safe
# whenever RAM is free, and it is the difference between a clean build and a mysterious signal 9.
_SWAP_PCT=$(_swap_used_pct)
if [ "$_SWAP_PCT" -ge 90 ]; then
  printf '\033[33m  ⚠ swap is %s%% full — stale pages from an earlier spike.\033[0m\n' "$_SWAP_PCT" >&2
  printf '\033[33m    RAM is otherwise free; cycle it with:  sudo swapoff -a && sudo swapon -a\033[0m\n' >&2
fi

printf '\033[2m  mem-guard: %s MB available → CARGO_BUILD_JOBS=%s (of %s cores)\033[0m\n' \
  "$_MEM_MB" "$CARGO_BUILD_JOBS" "$_NPROC" >&2

# ── sccache, opt-in and self-disabling.
#
# The wrapper must NEVER be a committed hard dependency: `.cargo/config.toml` is shipped to every clone,
# and cargo does not degrade when the wrapper is absent — it dies with
# `could not execute process \`sccache ... rustc -vV\``. That one committed line broke **every CI job on
# every OS**, and never once locally, because sccache is installed here. *A committed artifact must be
# usable by anyone who clones this repo.*
#
# So: use it when it is there, and be silent when it is not.
if command -v sccache >/dev/null 2>&1; then
  export RUSTC_WRAPPER="${RUSTC_WRAPPER:-sccache}"
fi
