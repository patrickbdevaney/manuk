#!/usr/bin/env bash
# Ephemeral build output in RAM (METHODOLOGY 10.1).
#
# This is a DISK-WEAR elimination, not a speed win. Two independent benchmarks found near-zero
# build-time difference between tmpfs and SSD, because rustc/LLVM codegen is CPU-bound. Do not let a
# future audit credit this with a performance improvement it did not produce. What it buys is that
# the tick loop stops grinding gigabytes of regenerable object code into a finite-endurance SSD.
#
# ## What actually churns
#
# The three things in `target/` have completely different write profiles, and conflating them is how
# you end up moving 12GB into RAM to save writes that were never happening:
#
#   dependency rlibs   ~10GB   written ONCE, then read forever.        Zero churn. Belongs on disk.
#   our crates + bins  ~0.5GB  rewritten every build.                  Churns.
#   incremental/       ~1-3GB  rewritten on EVERY EDIT, thousands of   Churns hardest, by far.
#                              small files, and thrown away anyway.
#
# `incremental/` is both the dominant write source and the least valuable output — `disk-hygiene.sh`
# already deletes it on sight. Putting *that* in RAM captures the great majority of the wear for
# 1-3GB of memory instead of 25GB. That is the default mode here, and it is free: no rebuild, no
# risk, works on any machine.
#
# ## The all-in-RAM mode, and why it refuses
#
# `--full` points `CARGO_TARGET_DIR` at the tmpfs so that every compile and every test binary lives
# in RAM. 10.1 says to size this empirically on the real machine rather than picking a round number,
# and on a 31GB box with a 12GB release target that arithmetic does not close: you would be leaving
# ~7GB for parallel LLVM codegen with mozjs in the mix. So this mode PRECHECKS and refuses out loud
# rather than trading a disk-wear problem for an OOM, which is a strictly worse failure — ENOSPC is
# a build error, OOM is a machine that stops answering.
#
# The tmpfs is size-capped, which makes the safety a property of the mount and not of our restraint:
# overflow surfaces as ENOSPC from the kernel, loudly, at the compiler. It cannot silently eat RAM.
#
# Banked releases are unaffected: `bank-release.sh` copies the binary out to ~/manuk-builds on real
# disk, which is exactly the persistence exception 10.1 carves out.
#
#   scripts/ramdisk.sh            # incremental → RAM  (default; safe anywhere)
#   scripts/ramdisk.sh --full     # whole target → RAM (prechecks; refuses if the RAM isn't there)
#   scripts/ramdisk.sh --flush    # drop RAM build output now
#   scripts/ramdisk.sh --status
set -uo pipefail
cd "$(dirname "$0")/.."

RAM_ROOT="${MANUK_RAMDISK:-/dev/shm/manuk-build}"
# Headroom for parallel LLVM codegen. mozjs's bindgen + codegen units are the pig; this is not a
# round number for its own sake, it is what the largest crate in the tree actually wants.
HEADROOM_GB="${MANUK_RAM_HEADROOM_GB:-8}"
# Watermark for the incremental cache. Incremental grows without bound across feature-flag
# permutations (stylo/minimal, spidermonkey/none) — each one keeps its own fragments forever.
INCR_CAP_MB="${MANUK_INCR_CAP_MB:-4096}"

avail_gb() { free -g | awk 'NR==2 {print $7}'; }
dir_mb()   { du -sm "$1" 2>/dev/null | cut -f1 || echo 0; }

status() {
  echo "▶ RAM build root: $RAM_ROOT"
  if [ -d "$RAM_ROOT" ]; then
    echo "    in RAM:      $(du -sh "$RAM_ROOT" 2>/dev/null | cut -f1)"
  else
    echo "    in RAM:      (not created)"
  fi
  echo "    /dev/shm:    $(df -h /dev/shm | awk 'NR==2 {print $3" used of "$2}')"
  echo "    RAM free:    $(avail_gb)G available of $(free -g | awk 'NR==2 {print $2}')G"
  for p in target/debug/incremental target/release/incremental; do
    if [ -L "$p" ]; then echo "    $p → $(readlink "$p")  [IN RAM]"
    elif [ -d "$p" ]; then echo "    $p  [on disk, $(dir_mb "$p")MB]"; fi
  done
}

# Flush is a DISCIPLINE, not a cleanup: RAM that is never reclaimed is just a slower OOM. Every mode
# below calls this, and disk-hygiene.sh calls it too, so the ceiling is enforced continuously rather
# than at the moment someone remembers.
# Wipe, then IMMEDIATELY re-create the directories the symlinks point at.
#
# Not a tidiness detail — it is the whole safety property. `rm -rf $RAM_ROOT/*` removes the
# `*-incremental` directories themselves, and `target/<prof>/incremental` is a symlink INTO one of
# them. A symlink to a directory that no longer exists is not an empty directory; it is a dangling
# path, and cargo does not recover from it:
#
#     error: couldn't prepare build directories
#     Caused by: failed to create directory `target/release/incremental`
#     Caused by: File exists (os error 17)
#
# The build simply stops, with an error that names the filesystem and not the cause. I wrote the
# flush, ran it, and broke the next build with it — so the recreate lives INSIDE flush, where it
# cannot be forgotten by a caller, rather than in the callers, where it already was.
reseat() { mkdir -p "$RAM_ROOT/debug-incremental" "$RAM_ROOT/release-incremental"; }

flush() {
  local used
  used=$(dir_mb "$RAM_ROOT")
  # NEVER delete *-incremental under a live compile: rustc hard-errors mid-flight ("failed to move
  # dependency graph … os error 2") and every in-flight crate loses its incremental state. This exact
  # call, cron-fired every 3min via disk-hygiene, was the 93s→189s→694s wall regression (journal,
  # tick-325 escalation). These fragments live in RAM — flushing frees RAM, not disk — so the only
  # reason to proceed mid-compile is imminent OOM.
  if pgrep -x rustc >/dev/null 2>&1 || pgrep -x cargo >/dev/null 2>&1; then
    local avail_kb
    avail_kb=$(awk '/MemAvailable/{print $2}' /proc/meminfo 2>/dev/null)
    if [ "${avail_kb:-99999999}" -gt 4194304 ]; then
      echo "  · flush SKIPPED — compiler live (${used}MB in RAM; mid-compile deletion hard-errors builds)"
      return
    fi
    echo "  · compiler live BUT MemAvailable <4G — flushing anyway (OOM beats one broken compile)"
  fi
  if [ "${1:-}" = "--force" ]; then
    echo "  · flushing all RAM build output (${used}MB)"
    rm -rf "${RAM_ROOT:?}"/* 2>/dev/null
    reseat
    return
  fi
  if [ "$used" -gt "$INCR_CAP_MB" ]; then
    echo "  · RAM build output is ${used}MB, over the ${INCR_CAP_MB}MB cap — flushing"
    echo "    (incremental fragments accumulate one set per feature-flag permutation and are"
    echo "     never read again; the cost of dropping them is one slower compile)"
    rm -rf "${RAM_ROOT:?}"/* 2>/dev/null
    reseat
  else
    echo "  · RAM build output ${used}MB / ${INCR_CAP_MB}MB cap — within budget"
  fi
}

case "${1:-}" in
  --status) status; exit 0 ;;
  --flush)  mkdir -p "$RAM_ROOT"; flush --force
            for prof in debug release; do
              mkdir -p "target/$prof"
              ln -sfn "$RAM_ROOT/$prof-incremental" "target/$prof/incremental"
            done
            status; exit 0 ;;
esac

mkdir -p "$RAM_ROOT"

if [ "${1:-}" = "--full" ]; then
  target_gb=$(( $(dir_mb target) / 1024 ))
  need=$(( target_gb + HEADROOM_GB ))
  have=$(avail_gb)
  echo "▶ all-in-RAM precheck: target/ is ${target_gb}G, +${HEADROOM_GB}G codegen headroom = ${need}G needed; ${have}G available"
  if [ "$have" -lt "$need" ]; then
    cat >&2 <<EOF

  REFUSED. ${have}G available, ${need}G needed.

  Moving the whole build tree into RAM here would leave too little for parallel LLVM codegen —
  mozjs in particular — and the failure mode of getting this wrong is not a slow build, it is an
  OOM kill. METHODOLOGY 10.1 is explicit that an undersized mount "risks swapping or OOM instead
  of helping", and that sizing is to be verified empirically on the real machine. It has been, and
  it does not fit.

  Run without --full: \`incremental/\` goes to RAM instead. That is where essentially all of the
  write churn is (rewritten on every edit; deleted by disk-hygiene anyway), for 1-3G instead of
  ${target_gb}G. The dependency rlibs that make up the bulk of target/ are written once and read
  forever — putting them in RAM buys no wear reduction at all, only rent.

EOF
    exit 1
  fi
  echo "  export CARGO_TARGET_DIR=$RAM_ROOT/target"
  export CARGO_TARGET_DIR="$RAM_ROOT/target"
  flush
  status
  exit 0
fi

# Default: the churn goes to RAM, the archive stays on disk.
#
# Cargo has no env var for the incremental directory, but it does not care that the directory is a
# symlink — it creates fragments through it exactly as it would on disk. The symlink is recreated on
# every run because tmpfs does not survive a reboot, and a dangling symlink IS a build failure, so
# this script is idempotent and safe to call from verify.sh unconditionally.
echo "▶ incremental fragments → RAM (the dominant write source; regenerable; deleted anyway)"
for prof in debug release; do
  disk="target/$prof/incremental"
  ram="$RAM_ROOT/$prof-incremental"
  mkdir -p "target/$prof" "$ram"
  if [ -L "$disk" ]; then
    # Re-point it: after a reboot the symlink survives on disk but its target does not.
    ln -sfn "$ram" "$disk"
  elif [ -d "$disk" ]; then
    sz=$(dir_mb "$disk")
    rm -rf "$disk"
    ln -sfn "$ram" "$disk"
    echo "  · target/$prof/incremental: ${sz}MB reclaimed from disk, now in RAM"
  else
    ln -sfn "$ram" "$disk"
    echo "  · target/$prof/incremental → RAM"
  fi
done

flush
echo
status
