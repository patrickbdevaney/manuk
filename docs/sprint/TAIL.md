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
| https://atendimentomondial.zendesk.com/ | 1 | 7 | 87 | 6 | ORIGIN | live | origin answered 403 (bot-wall-403) to our client in the fidelity dump; unmeasurable, not a rendering failure |
| https://littlecaesarsbcs.libellum.com.mx/ | 2 | 97 | 7 | 96 | ENGINE | runs | drive 7% with SHAPE 97, a11y 96 |
| https://chat.google.com/ | 4 | 88 | 9 | 99 | ENGINE | runs | drive 8.8% over 91 targets, shape 88, a11y 99: same drive symptom as livescore |
| https://webfenix.movilidadbogota.gov.co/ | 2 | 11 | 100 | 86 | ORIGIN+ENGINE | live | UNREACHABLE in runs 1 and 4 (flapping); when measured, UNMEASURABLE render-failed (ours) and 40/45 misses at dx=184 dy=100 |
| https://sestra.cc/ | 4 | 93 | 99 | 13 | ENGINE | runs | a11y F1 12.5 (prec 9.6) with SHAPE 93 and drive 99 |
| https://crm.majoo.id/ | 1 | 61 | 14 | 67 | ENGINE | live | structural 92.5% (oracle 53 paths, ours 49, 4 missing); 19/49 misses, login form children width 26 (Chrome) vs 404 (ours); font metrics agree |
| https://allticketscol.com/ | 4 | 67 | 15 | 83 | INSTRUMENT | live | oracle 80 paths vs ours 1767 paths; Chrome body 1200x921 vs ours 1200x7005 (Angular app: oracle captured a pre-render shell; unconfirmed without a second capture) |
| https://experiencia.pichincha.com/ | 4 | 17 | 100 | 86 | ENGINE | live | ours 22 vs oracle 37 paths; heading font 72/14 vs 72/21 and y offsets of 50-80px |
| https://www.ikea.com/ | 4 | 64 | 17 | 93 | ENGINE | live | structural 80.7% (169 missing of 874); 264/264 misses disagree about font advance; astro-island containers 684px tall in Chrome, 0 in ours |
| https://nortenoticia.com.br/ | 2 | 42 | 17 | 90 | ENGINE | live | marquee with 255 anchors: Chrome lays them out in a row (x up to +27164), ours stacks them at x=0 (y 12193); 421 of 695 misses |
| https://redinfor.com.pe/ | 2 | 83 | 20 | 54 | ENGINE | live | section chain 1200x800 in Chrome, 1200x0 in ours (height +800 on 3 top misses); BOOT mCustomScrollbar is not a function |
| https://www.paypal.com/ | 3 | 66 | 28 | 71 | ENGINE | live | structural 88.5% (72 missing of 625); page height Chrome 18640 vs ours 11883; section:nth-of-type(10) y -9115 |
| https://meuconsultordigital.com.br/ | 4 | 45 | 53 | 96 | ENGINE | live | 327 of 524 scored are misses; y offset +1662 accumulating from div:nth-of-type(8), body 9460 vs 7858 |
| https://www.tz.de/ | 1 | 89 | 46 | 84 | ENGINE | live | nav ol is 83px wide in Chrome vs 1908 in ours; 17 list-item->block display disagreements; 200 of 1861 misses all font advance |
| https://admin.zoomph.com/ | 4 | 59 | 100 | 50 | ENGINE | live | SHAPE 58.8: 14/34 misses, 50px height deltas on the login card; a11y NAVFAIL in run 1 only (ORIGIN) then 66.7 |
| https://www.smecel.com.br/ | 2 | 53 | 80 | 100 | ENGINE | live | ul height 589 (Chrome) vs 166 (ours); 5 list-item->block display disagreements; nav ul width 576 vs 901 |
| https://www.wdimax.com/ | 4 | 97 | 54 | 100 | ENGINE | live | only 9 of 377 scored miss, all Segoe UI advance 161 vs 157; BOOT syntax-error at inline#1021 |
| https://www.monopolybingogame.com/ | 1 | 60 | 56 | 78 | ENGINE | live | structural 9.3%: 245 of 270 oracle paths missing, ours 177; main wrapper 2460px tall in ours vs 72 in Chrome; 2 block->flex display disagreements |
| https://portagelearning.edu/ | 3 | 56 | 86 | 86 | INSTRUMENT | live | oracle 177 paths vs ours 530; cov 14.1; SHAPE 56 over 25 scored with 0 dx/dy and 3px dh |
| https://hnhbkis.edu.in/ | 1 | 100 | 57 | 72 | ENGINE | live | shape dump is near clean (1 of 314 scored misses, height +20), so the sub-75 is the drive score (57), whose cause this dump does not show |
| https://secure5.entertimeonline.com/ | 4 | 67 | 100 | 62 | ENGINE | live | 12/39 misses all font advance, y offsets of +48 on the aside |
| https://mobcup.fm/ | 4 | 100 | 100 | 64 | ENGINE | live | structural 88.2%: 4 of 34 oracle paths missing, 0 shape misses; only the a11y F1 (64) is low |
| https://www.freesupertips.com/ | 2 | 87 | 66 | 92 | ENGINE | live | 45 of 101 misses are display disagreements (list-item->block x44); header container 242px tall in Chrome vs 0 in ours |
| https://portal.ensuretyfinance.com/ | 4 | 73 | 100 | 86 | ENGINE | live | only 3 of 22 scored miss, button width 132 vs 153 (-21); the sub-75 is SHAPE 73 over only 22 scored elements |
| https://serennu.com/ | 4 | 82 | 94 | 74 | ENGINE | live | 11 of 61 miss with height -12 on body/center (ours 12px taller); the sub-75 is the a11y F1 74, not shown by this dump |
| https://promo.golesliga1max.pe/ | 2 | 75 | 100 | 96 | ENGINE | live | input width 219 in Chrome vs 410 in ours (-191); page height 1544 vs 1525 |
| https://www.kicktipp.com/ | 4 | 92 | 100 | 75 | ENGINE | live | 7 of 93 misses, Arial 15 advance disagreements and y +15 on the content column |

## Counts by class (class, evidence): site count

- ENGINE / live: 23
- ENGINE / runs: 4
- INSTRUMENT / live: 7
- ORIGIN / live: 2
- ORIGIN / runs: 2
- ORIGIN+ENGINE / live: 1
- ORIGIN+ENGINE / runs: 1
