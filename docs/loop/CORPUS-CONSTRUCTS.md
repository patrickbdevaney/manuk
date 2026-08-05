# CORPUS CONSTRUCTS — what the burndown's own corpus is actually made of

**Measured at tick 965 against `docs/bench/corpus-crux-trend.txt`**, the 200-site representative CrUX
sample that produces M1 — the number the loop is scored on.

## Why this file exists

The loop ranks work by *usage-weight × failing-breadth* (CONSTITUTION VI.3). It has never had a
number for the first term **on the corpus that scores it**, so "usage-weight" has been an argument
from the open web while the metric was computed over 200 specific pages. Those are not the same
population, and t962/t963 landed two Chrome-exact, RED-proven capability fixes for constructs that
appear on **zero** of these pages.

> **A fix for a construct the corpus does not contain cannot move the corpus, however right it is.**
> That is not an argument against the fix — I4 ranks the real web, not this sample — but it is a
> complete explanation of a flat metric, and it is available for one command *before* the work rather
> than as a puzzle after it.

## How to re-measure (one command, ~3 minutes, no build)

```bash
awk 'NF{print $NF}' docs/bench/corpus-crux-trend.txt | grep '^http' > /tmp/urls.txt
mkdir -p /tmp/corpsnap
cat /tmp/urls.txt | xargs -P 12 -I{} sh -c 'u="{}"; n=$(echo "$u" | md5sum | cut -c1-12);
  curl -sL --max-time 12 -A "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36" \
  "$u" -o "/tmp/corpsnap/$n.html"'
# then grep /tmp/corpsnap for the construct, over files larger than 500 bytes
```

**171 of 200 came back with a real body** (>500 bytes); the rest are the bot-walls and empty-2xx
origins the sweep already excludes, so the denominator below is the same population the metric scores.

## The ranking (tick 965)

```text
   95/171   55.6%  ██████████████████████                   <button>
   88/171   51.5%  ████████████████████                     <input>
   84/171   49.1%  ███████████████████                      @media
   79/171   46.2%  ██████████████████                       display:flex
   59/171   34.5%  █████████████                            transform:
   59/171   34.5%  █████████████                            <svg> inline
   54/171   31.6%  ████████████                             CSS custom property var()
   52/171   30.4%  ████████████                             <iframe>
   51/171   29.8%  ███████████                              calc()
   46/171   26.9%  ██████████                               <picture> / srcset
   35/171   20.5%  ████████                                 @font-face
   32/171   18.7%  ███████                                  display:grid
   19/171   11.1%  ████                                     aspect-ratio
   19/171   11.1%  ████                                     <select>
   18/171   10.5%  ████                                     text-overflow:ellipsis
   18/171   10.5%  ████                                     overflow:auto|scroll
   15/171    8.8%  ███                                      clamp() / min() / max()
   12/171    7.0%  ██                                       writing-mode / dir=rtl
   12/171    7.0%  ██                                       <table> with <tr> rows
   11/171    6.4%  ██                                       position:sticky
   10/171    5.8%  ██                                       <video>
   10/171    5.8%  ██                                       <details> / <summary>
    9/171    5.3%  ██                                       @supports
    7/171    4.1%  █                                        <dialog>
    6/171    3.5%  █                                        -webkit-line-clamp
    5/171    2.9%  █                                        <textarea>
    5/171    2.9%  █                                        <td colspan|rowspan>
    3/171    1.8%                                           <pre>
    2/171    1.2%                                           shadow DOM / custom elements
    2/171    1.2%                                           <canvas>
    0/171    0.0%                                           <select multiple> / size>=2
    0/171    0.0%                                           a TAB inside a <pre>
```

## ⚠ How to read it, and the one way it lies

**Markup rows are solid; CSS rows are LOWER BOUNDS.** This greps the HTML a `curl` returns, so a
`<button>` count is exact and a `display:flex` count sees only what is inline or in a `<style>`
block — every external stylesheet is invisible to it. **46.2% for flex is a floor, not a level.** Any
comparison between a markup row and a CSS row is therefore unsound in one direction only, and the
conclusions below are stated between markup rows or between CSS rows.

It also measures **presence, not weight**: a page with one `<button>` and a page whose whole layout is
buttons both count once. Presence is the right first filter (it answers *"can this fix ever
appear?"*) and the wrong final ranking.

## What it says, on the evidence

⚠⚠⚠ **FORM CONTROLS ARE THE #1 AND #2 CONSTRUCTS IN THIS CORPUS.** `<button>` 55.6% and `<input>`
51.5% beat every other markup row measured, and they beat `<table>` **eight to one**. t963 established
the *shape* of the defect class — **a form control whose intrinsic size model is ABSENT rather than
wrong**, invisible to every fixture that sets a size — and found it on the control that turns out to be
the corpus's fourteenth most common, not its first two.

⚠⚠ **VI.2's RESIDUE ORDER IS CORRECTED BY THIS.** The H0.1 row ranks the remaining mass as *tables ·
inline composition · scroll containers*. In this corpus **tables are 7.0%** and `<td colspan>` is
2.9%, while the form-control rows are 51–56%. Tables remain real work; they are not the top of a
usage-weighted list computed over the pages that produce the number.

⚠⚠ **AND IT PRICES THE LAST TWO CAPABILITY TICKS AT ZERO, IN ADVANCE.** `<select multiple>` and a tab
inside a `<pre>` are each **0 of 171**. t962 and t963 are Chrome-exact and RED-proven; they are also
structurally incapable of moving M1 on this corpus. Both were reached from a surface audit, which
ranks against the *web*, and neither was checked against the *corpus* first — one command that did
not get run.

## The standing rule this file creates

> **Before building a fix that is meant to move the burndown, grep the corpus for the construct.**
> A zero is not a reason to abandon the work — I4 ranks the real web and the surface audit is the
> instrument for that — but it *is* a reason to stop expecting the metric to show it, and to say so
> in the tick rather than in the next sweep's post-mortem.
