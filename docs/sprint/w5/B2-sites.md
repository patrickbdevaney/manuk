# W5.B2.a WebArena-Verified disk + runtime plan (chrome_ver=145.0.7632.116 pin, no browser measured here)

Source: ServiceNow/webarena-verified docs/environments (optimized `am1n3e/webarena-verified-*` Docker Hub images, Feb 2026) and Docker Hub tag API (compressed registry size, amd64). Nothing pulled.
Task coverage: hard subset ids (`data/B2-hard-ids.json`, 258) joined to `sites` in the full dataset (`data/B2-hard-sites.json`).

| site | on-disk image GB | registry (compressed) GB | external data GB | hard tasks touching |
|---|---|---|---|---|
| shopping | 13.3 | 5.42 | - | 61 |
| shopping_admin | 2.9 | 1.25 | - | 57 |
| reddit | 8.41 | 4.57 | - | 65 |
| gitlab | 31.6 | 22.0 | - | 81 |
| wikipedia | 0.115 | 0.05 (arm64 only listed) | ~100 download | 23 |
| map | 3.28 | 1.19 | ~60 download (docs: min 200 GB disk) | 19 |

Hard-subset task site sets: gitlab 57, shopping 56, shopping_admin 55, reddit 42, gitlab+reddit 18, map+wikipedia 17, gitlab+wikipedia 6, reddit+shopping 5, map+shopping_admin 2.

## Chosen subset: shopping, shopping_admin, reddit, gitlab
- Covers 233 of 258 hard tasks; 25 wikipedia/map tasks deferred (needs ~160 GB of data downloads).
- Need 56.2 GB. `/` has 132.8 GB free (docker data-root `/var/lib/docker` is on `/`) minus 25 GB reserve = 107.8 GB budget. `/home` budget 100.3 GB.
- Adding map: 119.5 GB > 107.8 GB, gate exits 1. Adding wikipedia: worse.
- Gate: `python3 -I docs/sprint/w5/B2-sites.py` (exit 0 chosen; `--add map` exit 1).
- Caveat: on-disk sizes are the maintainers' published figures, not measured here; recheck after the first pull in W5.B2.b with `docker system df`.

## Runtime decision (needs owner OK, not done)
No docker/podman on the box. Options: (a) docker via sudo apt (system change, shared box); (b) rootless podman, data-root under `~/.local/share/containers` on `/home` (100 GB budget still fits 56 GB, but `/home` is 92% used). Recommendation: rootless podman with graphroot on `/` via a configured path, avoiding a daemon and sudo-run containers. Needs owner confirmation; W5.B2.b is gated on it.
