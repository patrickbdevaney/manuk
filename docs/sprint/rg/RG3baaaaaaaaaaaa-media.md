## RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a

chrome_ver=145.0.7632.116. Why opendining's date picker was a "Select date" Cancel/OK modal on Chrome but a closing popup on manuk.

- Cause: headless Chrome has no pointing device, so the page read `(pointer: none)=true`, `(pointer: fine)=false` (hover: none likewise); manuk answers `pointer: fine`, `hover: hover`. MUI picks the mobile modal picker when `(pointer: fine)` is false, so the Chrome backend was served a different variant (dialog "Select date", Cancel + OK, stays open after a day click). Viewport values were identical (innerWidth 1280, max-width queries equal).
- Fix: `mcp/src/cdp.rs` `chrome_args` adds `--blink-settings=primaryPointerType=4,availablePointerTypes=4,primaryHoverType=2,availableHoverTypes=2`, i.e. what desktop Chrome reports.
- Probe `bench/agent-tasks/rg3baaaaaaaaaaaa_media.py`: before the fix media differed (pointer fine vs none) and picker differed (chrome dialog 'Select date', Cancel/OK true; manuk none); after, all 17 printed values are identical on both backends and after replay step #6 neither shows a dialog (picker closed on the day click on both). Red check: `diff-picker` mutant (identical media, differing picker) exits 1.
- Unit test `cdp::pointer_flag_tests`; `cargo test -p manuk-mcp` all pass.
