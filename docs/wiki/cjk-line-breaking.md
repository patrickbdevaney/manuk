# CJK line breaking — `line-break`, and why a correct library gives the wrong answer

## The one-sentence version

**UAX #14's *default* algorithm is CSS's `line-break: strict`, and CSS's *initial value* is `auto`.**
An engine that asks a conformant UAX #14 library where a run may break, and uses the answer, is in
`strict` mode on every Japanese page it renders — without a flag, a warning, or a field to notice was
never written.

## What CJ is, and why it is most of the language

UAX #14 LB1 resolves the class **CJ** (*Conditional Japanese Starter*) to **NS** (*non-starter*),
which forbids a line from **beginning** with the character. CJ is:

- the small kana — `ぁ ぃ ぅ ぇ ぉ っ ゃ ゅ ょ ゎ ヵ ヶ` and their katakana forms,
- the prolonged sound mark `ー` (U+30FC),
- the halfwidth katakana small letters `ｧ ｨ ｩ … ｯ ｰ` (U+FF67–U+FF70).

`っ` and `ー` appear in a large fraction of ordinary Japanese sentences. Under the untailored
default, a line that would have ended just before one instead ends a character earlier — so the
paragraph is a character narrower on that line, and **every element below it inherits the difference
as `dy`**. It is not a visible bug on any single line; it is a systematic shape error on a whole
class of the web, and it is invisible to any test written in Latin.

## The four values, measured

Chrome, headless, on WPT's own fixture markup (`文文文文文文<X>字<span>字</span>` in a 185px box at
30px/1em, against a `<br>`-forced reference). `÷` = a line may begin with the character, `·` = it may
not; each cell is `auto loose normal strict`:

```text
  class / char                        lang=ja  lang=zh  lang=de  no lang
  CJ     ぁ っ ー ｧ ｰ                    ÷÷÷·     ÷÷÷·     ÷÷÷·     ÷÷÷·
  ITER   々 〻 ゝ ヽ                      ·÷··     ·÷··     ·÷··     ·÷··
  IN     ‥ … ⋯ ︙                        ····     ····     ····     ····
  PR-AFW ± € № ﹩ ＄ ￡ ￥ ￦             ÷÷÷÷     ÷÷÷÷     ÷÷÷÷     ÷÷÷÷
  CPM    ・ ： ； ･ ‼ ⁇ ⁈ ⁉ ！ ？           ·÷··     ·÷··     ····     ····
  PO-AFW ° ‰ ′ ℃ ﹪ ％ ￠               ·÷··     ·÷··     ····     ····
  HYPH   〜 ゠                           ·÷÷·     ÷÷÷·     ····     ····
  HYPH   ‐ –                            ·÷··     ·÷··     ····     ····
  ID     一                     CTRL     ÷÷÷÷     ÷÷÷÷     ÷÷÷÷     ÷÷÷÷
  CL     、 。 ）                CTRL     ····     ····     ····     ····
  OP     （                     CTRL     ÷÷÷÷     ÷÷÷÷     ÷÷÷÷     ÷÷÷÷
  PR-Na  $ +                    CTRL     ÷÷÷÷     ÷÷÷÷     ÷÷÷÷     ÷÷÷÷
  PO-Na  %                      CTRL     ····     ····     ····     ····
```

Three things this table says that the spec text does not:

1. **There is a vertical seam.** `CJ` and the iteration marks are properties of the *character*.
   `CPM`, `PO-AFW` and the two hyphen rows are properties of the *content language* — Chrome applies
   them only when the text is Japanese or Chinese. A tailoring measured against `lang="ja"` alone
   looks language-independent and is not.
2. **`auto` is not `normal`.** The `〜 ゠` row is the only place it shows, and it differs between
   Japanese and Chinese as well. `auto` is UA-defined, so this is Chrome's choice rather than the
   spec's, which is exactly why it has to be measured.
3. **Chrome refuses two rules CSS Text §5.2 states.** `loose` is specified to allow a break before an
   inseparable character (`…`); `normal`/`strict` are specified to forbid one before a prefix with
   East Asian Width A/F/W (`±`, `€`). Chrome does neither. WPT's `-in-loose` and
   `-pr-{normal,strict}` therefore fail in Chrome too.

## The implementation shape: tailor the INPUT

`engine/layout` — `line_break_probe`. For a run that contains something to re-class, it builds a
**parallel string** with each re-classed character swapped for an ideograph (`一`) and runs
`unicode-linebreak` over that, mapping the byte offsets back through a per-character map.

Rewriting the input rather than post-filtering the crate's opportunities is what keeps the **context**
correct: an open bracket still forbids a break after it, a close bracket still forbids one before it,
`NU × ID` still holds. Editing the answer per opportunity means re-deriving the pair table by hand,
and the `OP`/`CL` control rows above are what would catch you having got it wrong.

Both strings have the same **character** count and not the same **byte** length (`°` is two UTF-8
bytes, `一` is three), so the offset map is kept per character rather than assumed to be the identity.

The probe returns `None` — no allocation, original string scanned — when the word is ASCII, or when
nothing in it is re-classed. That is nearly every word on nearly every page.

## What is deferred, and on what

The four `NEEDS lang` groups need a **content language**, and the engine has no notion of one: no
`lang` attribute inheritance, no `:lang()`, no `<meta http-equiv="content-language">`. They are
deferred **as a unit** rather than approximated, because an unconditional version is wrong for every
German page that quotes a `％` — which is not a hypothetical: shipping them unconditionally turns six
`other-lang`/`unknown-lang` WPT files red while the area total still climbs.

## The trap this belongs to

A **net gain can hide a regression**, and a total cannot see it. The unconditional version scored
`+432` on `css/css-text/i18n` and had reddened six previously-green files; the language-gated version
scores the same `+432` with the remaining failures a strict **subset** of the ones it started from.
The check that distinguishes them is diffing the failing **name lists**, not the counts.

## See also

- `docs/wiki/box-layout.md` — where inline runs become line boxes.
- `engine/layout/src/lib.rs` — `break_segments`, `line_break_probe`,
  `line_break_tailors_the_cjk_classes_the_way_chrome_does`.
- WPT `css/css-text/i18n/{ja,zh,other-lang,unknown-lang}/` — the four-language matrix.
