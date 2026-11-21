# W4.B4.a.a — navigate observation bytes (chrome_ver=145.0.7632.116)

No new engine change: the W5.T1 lean observe (landed after the B4a sample) already cuts navigate bytes. Own suite x3 (120 runs/arm, haiku), turn-log result_bytes of navigate calls per task: lean 4303 vs legacy (`MANUK_OBSERVE_MUTATE=legacy`, the revert) 6821 = -36.9%; B4a baseline 7035 (n=10, pre-lean). Passes 117/120 vs 116/120. books.toscrape navigate: 20568 B (B4a) -> 7859 B.
Gate `bench/agent-tasks/b4aa_gate.py`: exit 0; `--mutate swap` (legacy as the lean arm) exit 1. See GATE.txt.
Not measured here: in+out tokens vs playwright+preload (follow-up row).
