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

⚠⚠⚠ **THE JOIN KEY, AND IT COST A 5× UNDERCOUNT AT t1092.** A CSS construct lives in the
*stylesheets*, not the HTML, so the method above must be extended to fetch each page's
`<link rel=stylesheet>` and join them back to the page. **`echo "$u" | md5sum` appends a NEWLINE.**
If the fetch loop names files that way and the analysis hashes the bare URL, 363 of 384 stylesheets
silently fail to join, the construct is looked for in the HTML alone, and the answer comes back as a
plausible small number: it read **21 of 170 (12%)** for a construct that is at **116 of 170 (68%)**.
The failure mode is a QUIET undercount that reads as a negative result and retires the best lever on
the board. *A join key computed two ways is two keys.*

## Priced constructs — the pseudo-element family (t1094)

See `docs/loop/CORPUS-PSEUDO-t1094.tsv`. ⚠ **Two of the seven were ALREADY IMPLEMENTED**, both found
by probing before building rather than after — clearfix (30%) and `content: attr()` (25%). Usage
weight ranks where to LOOK; only a probe says whether anything is THERE.

⚠ **`clip: rect()` is 36% of pages and M1 is structurally blind to it** — `clip` is paint-time, so
Chrome and Manuk report the SAME box, and the sr-only idiom is already invisible from
`width:1px;height:1px;overflow:hidden`. A high usage-weight number does not imply a scoreable one.

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

### ⚠⚠⚠ The multiplier on that floor is ~3×, measured (tick 998) — and it is removable

The caveat above was stated qualitatively for 33 ticks. Tick 998 needed a CSS-property number it could
defend, fetched **each site's linked stylesheets as well**, and measured both ways on the same
170-site population:

```text
   logical properties (margin-inline / inline-size / inset-block / …)
        HTML only ................  23/171   13.5%   <- this file's recipe
        HTML + its stylesheets ...  69/170   40.6%   <- the real level
   a universal `* { margin|padding: 0 }` reset
        HTML + its stylesheets ...  95/170   55.9%
   BOTH on the same site
        HTML + its stylesheets ...  44/170   25.9%
```

**A CSS row read off this file should be multiplied by roughly three before it is compared to
anything.** Better: stop needing the multiplier. The stylesheet stage is one extra `xargs` pass over
the same URL list and costs ~7 minutes; the marginal cost over the HTML-only crawl is the CSS fetch,
not a new instrument:

```bash
# after the HTML crawl above, per site: resolve <link rel=stylesheet> hrefs and fetch up to 8
grep -oiE '<link[^>]+stylesheet[^>]*>' index.html \
  | grep -oiE 'href=("[^"]*"|[^ >]+)' | sed -E 's/^href=//; s/"//g' | head -8
# resolve http* | //host | /path | relative against the page URL, then curl each into the site's dir
# and grep the site DIRECTORY (index.html + *.css), counting each site once
```

⚠ **What the 25.9% is and is not.** It is a **co-occurrence** — the construct pair is present on the
site — not a confirmed divergence. For a logical/physical cascade conflict to bite, both declarations
must reach the same element and the reset must be the one that should win. The honest bound on affected
sites is `0 ≤ n ≤ 44`. Presence remains the right first filter and the wrong final ranking, exactly as
the paragraph below says for markup.

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

## `border-collapse` (tick 999) — and the row that shows why a raw declaration count misleads

356 sites fetched with their linked stylesheets (the corrected recipe, not HTML alone):

```text
   border-collapse: collapse declared in the CSS   204/356   57.3%
   a <table> in the served HTML                     25/356    7.0%
   BOTH — where the defect can actually bite        20/356    5.6%
   any `border-style: hidden` anywhere               3/356    0.8%
```

⚠⚠⚠ **57.3% is the number a careless tick would publish, and it is the wrong one.**
`border-collapse` is in every CSS reset and every framework, so it is declared almost everywhere —
and it is **inert** unless the page also has a table. The honest figure is the *intersection*.

> **The rule this row adds: grep for the CONSTRUCT, not for the DECLARATION.** A property that is
> universally declared and conditionally applicable prices at its condition, not at its declaration.
> The same trap is waiting for `border-spacing`, `table-layout`, `caption-side`, `list-style`,
> `quotes`, `counter-reset` and every other property a reset sets pre-emptively on a selector that
> most pages never instantiate.

⚠ **5.6% is a floor twice over**: it counts only tables in the *served* HTML, so a JS-built table is
invisible to the grep; and co-occurrence is not confirmed divergence — the reset and the table must
also meet on the same element for the defect to be observable.
