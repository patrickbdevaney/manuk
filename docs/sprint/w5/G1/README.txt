G1/cdp-layer.tsv (cdp-layer) and G1/cdp-layer+pwfmt.tsv: the REAL rows for the two arms, chrome_ver=145.0.7632.116.
causes.tsv: one class per failed run. table.tsv: per-task pass counts. Regenerate with
python3 -I bench/agent-tasks/w5g1_report.py (needs target/real-runs transcripts from the run).

cdp-layer-v2.tsv (W5.G1.b.a.a, chrome_ver=145.0.7632.116) classed by W5.G1.b.b.a.a: causes-v2.tsv (arm a and b rows) and table-v2.tsv. Regenerate with: python3 -I bench/agent-tasks/w5g1_report.py --arm-tsv G1/cdp-layer-v2.tsv --causes-out G1/causes-v2.tsv --table-out G1/table-v2.tsv
