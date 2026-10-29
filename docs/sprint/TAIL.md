# TAIL.md - worst-site triage (node TAIL.1)

Source rows: `docs/bench/tri-sweep-{20261024-225412,20261024-232725,20261024-235659,20261025-002803}.tsv` (GUARD.R0 run1-4, chrome_ver=145.0.7632.116).
A site is listed when its mean over the runs where it was reachable is below 75 on SHAPE, drive or a11y F1: **40 sites** of 200 corpus rows in the runs.

Evidence levels: `live` = re-run today with `manuk-wpt fidelity --shape-dump 6` (logs in /tmp, side-by-side PNG read) plus curl/a11y-score --diff where noted; `runs` = inferred from the 4 persisted rows only; `none` = not yet examined. Only `live` rows are proven; `runs` rows are hypotheses.

Finding that governs several rows: the sweep's per-site numbers are identical across runs 1-4 for most sites (oracle and ours are cached per site), so ORIGIN variance shows only as reachability flapping, not as score noise.

| site | n_ok | SHAPE | drive | a11y F1 | class | evidence | note |
|---|---|---|---|---|---|---|---|
| https://booking.directferries.com/ | 4 | 50 | 0 | 25 | ENGINE | live | thin-overlap-2: oracle 20 paths, ours 2; absolute dh=800 (we paint an empty shell) |
| https://dashboard.twitch.tv/ | 4 | 53 | 0 | 15 | ENGINE | live | ours 58 paths vs oracle 159; Chrome has a full-viewport 1200x800 overlay where ours is 1200x40; nav item x +805 |
| https://esaj.tjsp.jus.br/ | 1 | 100 | 0 | 0 | ORIGIN | runs | reachable in 1 of 4 runs; the one measured run has cov 0.4 over 255 paths |
| https://hope.cap-systems.org/ | 4 | 0 | 100 | 100 | INSTRUMENT | live | stylesheets 302 to login.cap-systems.org without a session cookie (curl); Chrome oracle body at [8 8 1184] (unstyled, Times New Roman) vs ours [0 0 1200] (bootstrap applied) |
| https://linxonline.co.pierce.wa.us/ | 4 | 43 | 100 | 0 | ORIGIN | live | curl and our fetch both get a 768-byte 'IIS Windows Server' default page today; a11y-score --diff: ours 3 nodes vs Chrome 133 (Chrome saw the real site at oracle capture) |
| https://mexcbt.ed-cl.com/ | 3 | 0 | 100 | 86 | INSTRUMENT | live | every one of 25 misses is a font disagreement: Chrome Times New Roman/16 and zero-width flex children (stylesheet/module CSS not applied in oracle), ours Gill Sans styled |
| https://tracker.shadowfax.in/ | 4 | 0 | 100 | 86 | INSTRUMENT | live | UNMEASURABLE tree-divergence-48: oracle 13 paths, Times New Roman body (unstyled) vs ours 48 paths Roboto; only 1 element scored |
| https://www.amazon.com.mx/ | 1 | 0 | 0 | 0 | ORIGIN | runs | reachable in 1 of 4 runs, LOW_SAMPLE with 2 paths |
| https://www.cbse.gov.in/ | 2 | 30 | 100 | 0 | INSTRUMENT | live | SIDE.png: Chrome side has no background, broken images and Times New Roman; ours has the page's CSS; 44/63 misses are font Times vs -apple-system |
| https://www.jazzguitar.be/ | 4 | 1 | 22 | 33 | ENGINE | live | UNMEASURABLE render-failed: ours 3 paths vs oracle 485 (cov 0.6) in runs 2-4; run 1 measured cov 99.2 shape 2.7, so also flaky |
| https://ru4.bongacams-ru.com/ | 3 | 100 | 100 | 1 | ORIGIN+ENGINE | runs | a11y F1 1.1 (prec 37.5, rec 0.6) in all 3 reachable runs while SHAPE is 100: ours has few a11y nodes vs Chrome; reachable 3 of 4 |
| https://www.livescore.cz/ | 3 | 100 | 1 | 76 | ENGINE | runs | drive 1.3% over 1828 targets with SHAPE 100 and a11y 75.6: targets are not drivable in bulk |
| https://www.naukri.com/ | 4 | 55 | 90 | 2 | INSTRUMENT | live | oracle saw only 59 paths vs ours 445 (cov 30.5 is over the oracle's shell); a11y precision 1.0%, so ours is far larger than Chrome's tree |
| https://atendimentomondial.zendesk.com/ | 1 | 7 | 87 | 6 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://littlecaesarsbcs.libellum.com.mx/ | 2 | 97 | 7 | 96 | ENGINE | runs | drive 7% with SHAPE 97, a11y 96 |
| https://chat.google.com/ | 4 | 88 | 9 | 99 | ENGINE | runs | drive 8.8% over 91 targets, shape 88, a11y 99: same drive symptom as livescore |
| https://webfenix.movilidadbogota.gov.co/ | 2 | 11 | 100 | 86 | ORIGIN+ENGINE | live | UNREACHABLE in runs 1 and 4 (flapping); when measured, UNMEASURABLE render-failed (ours) and 40/45 misses at dx=184 dy=100 |
| https://sestra.cc/ | 4 | 93 | 99 | 13 | ENGINE | runs | a11y F1 12.5 (prec 9.6) with SHAPE 93 and drive 99 |
| https://crm.majoo.id/ | 1 | 61 | 14 | 67 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://allticketscol.com/ | 4 | 67 | 15 | 83 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://experiencia.pichincha.com/ | 4 | 17 | 100 | 86 | ENGINE | live | ours 22 vs oracle 37 paths; heading font 72/14 vs 72/21 and y offsets of 50-80px |
| https://www.ikea.com/ | 4 | 64 | 17 | 93 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://nortenoticia.com.br/ | 2 | 42 | 17 | 90 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://redinfor.com.pe/ | 2 | 83 | 20 | 54 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://www.paypal.com/ | 3 | 66 | 28 | 71 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://meuconsultordigital.com.br/ | 4 | 45 | 53 | 96 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://www.tz.de/ | 1 | 89 | 46 | 84 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://admin.zoomph.com/ | 4 | 59 | 100 | 50 | ENGINE | live | SHAPE 58.8: 14/34 misses, 50px height deltas on the login card; a11y NAVFAIL in run 1 only (ORIGIN) then 66.7 |
| https://www.smecel.com.br/ | 2 | 53 | 80 | 100 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://www.wdimax.com/ | 4 | 97 | 54 | 100 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://www.monopolybingogame.com/ | 1 | 60 | 56 | 78 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://portagelearning.edu/ | 3 | 56 | 86 | 86 | INSTRUMENT | live | oracle 177 paths vs ours 530; cov 14.1; SHAPE 56 over 25 scored with 0 dx/dy and 3px dh |
| https://hnhbkis.edu.in/ | 1 | 100 | 57 | 72 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://secure5.entertimeonline.com/ | 4 | 67 | 100 | 62 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://mobcup.fm/ | 4 | 100 | 100 | 64 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://www.freesupertips.com/ | 2 | 87 | 66 | 92 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://portal.ensuretyfinance.com/ | 4 | 73 | 100 | 86 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://serennu.com/ | 4 | 82 | 94 | 74 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://promo.golesliga1max.pe/ | 2 | 75 | 100 | 96 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |
| https://www.kicktipp.com/ | 4 | 92 | 100 | 75 | UNEXAMINED | none | no live dump yet; see TAIL.1.a |

## Counts by class (class, evidence): site count

- ENGINE / live: 5
- ENGINE / runs: 4
- INSTRUMENT / live: 6
- ORIGIN / live: 1
- ORIGIN / runs: 2
- ORIGIN+ENGINE / live: 1
- ORIGIN+ENGINE / runs: 1
- UNEXAMINED / none: 20
