# The text-field selection API — applicability, the resting caret, and the change detector

*Landed tick 1392. Every table below is headless-Chrome-measured; the arbitration probes are
reproduced inline so they can be re-run.*

`selectionStart` / `selectionEnd` / `selectionDirection` / `setSelectionRange` / `setRangeText` /
`select()` are how a page reads and moves the text cursor. Four separate rules govern them, and three
of the four are pairs — a single measurement is consistent with two different rules, and only the
second row tells you which one you have.

## 1. It applies to five input types and a `<textarea>`, and to nothing else

| element | `selectionStart` |
|---|---|
| `<textarea>` | `0` |
| `input` type `text` `search` `tel` `url` `password` | `0` |
| `input` with an **unrecognised** `type`, or **no** `type` attribute | `0` |
| `input` type `email` `number` `date` `month` `week` `time` `datetime-local` `range` `color` `checkbox` `radio` `file` `hidden` `submit` `image` `reset` `button` | **`null`** |

Two rows do the work here.

**`email` is excluded**, though it is a text field in every visual sense. An "is it texty?" predicate
lets it in; the spec keeps it out because `<input type=email multiple>` holds a *list*, and a list has
no single cursor. **The applicable set is a spec enumeration, not a category judgement** — you cannot
derive it, you have to carry it.

**An unrecognised keyword falls back to the Text state, so the API applies to it.** `<input
type="aninvalidtype">` *is* a text field. So the predicate cannot be "is the type one of the five"; it
is *resolve the keyword first, then ask* — which is why the implementation carries the complete
22-keyword list of what the spec knows, purely to detect that a keyword is **not** on it.

### Why `null` and not `0` matters

`if (el.selectionStart !== null)` is how a mask/caret/autocomplete library asks *"is this a field with
a cursor?"*. Answering `0` makes that YES for a spinner, a date picker and an email field, and the
library then computes an offset into a control that has none.

## 2. The setters throw `InvalidStateError` — except `select()`, which never throws

```js
numberInput.setSelectionRange(0, 1);   // InvalidStateError
numberInput.selectionStart = 0;        // InvalidStateError
numberInput.selectionDirection = 'none'; // InvalidStateError
numberInput.setRangeText('x');         // InvalidStateError
numberInput.select();                  // fine
```

"Select the contents" is meaningful for a spinner; "put the cursor at offset 3" is not. A copy-button
handed an arbitrary field calls `select()` and must not blow up.

> This is the same shape as [constraint validation](constraint-validation.md)'s `checkValidity()`
> returning `true` for a barred element whose `validity.valid` is `false`: **the family splits on which
> question is being asked, not on which element it is asked about.**

## 3. The caret RESTS at 0; a `.value` write is what moves it — *if the value changed*

A parsed `<input value="abcdef">` and a parsed `<textarea>abcdef</textarea>` both report
`selectionStart === 0`. The value's *length* is where the caret sits **after a script assigns
`.value`** — a consequence of the write, not the state of an untouched field.

| before | assignment | after |
|---|---|---|
| `"abcdef"`, sel `(1,3)` | `el.value = "zzzzzzzzzz"` | `10,10` — changed |
| `"abcdef"`, sel `(2,5)` | `el.value = "ab"` | `2,2` — changed, clamped |
| `"abcdef"`, sel `(2,4)` | `el.value = "abcdef"` | **`2,4` — unchanged, caret KEPT** |

⭐ **The third row is the rule.** Without it this reads as "assigning `.value` moves the caret to the
end" — a write hook. With it, it is a **change detector**.

That distinction is load-bearing for the entire controlled-component pattern. A React/Vue controlled
input re-assigns the field its current value on *every keystroke*; a write hook would slam the caret to
the end of the line on each character typed. It is the single most reported bug in every hand-rolled
controlled input, and this clause is why browsers do not have it.

The direction is deliberately **not** reset by the assignment.

## 4. `setRangeText`'s `preserve` mode asks a different question of each edge

The spec's two clauses look symmetric and are not. An edge **past** the replaced range moves by the
length delta — both edges. An edge landing **inside** the replaced range collapses to the range's
**start** for `selectionStart`, and to its **new end** for `selectionEnd`.

Measured on `"abcdefgh"`, replacing `[2,5)`:

| selection before | call | after | one shared closure gave |
|---|---|---|---|
| `(3,3)` | `setRangeText("Z",2,5)` | `2,3` | `2,2` ✗ |
| `(3,4)` | `setRangeText("Z",2,5)` | `2,3` | `2,2` ✗ |
| `(0,3)` | `setRangeText("Z",2,5)` | `0,3` | `0,2` ✗ |
| `(4,4)` | `setRangeText("ZZZZ",2,5)` | `2,6` | `2,2` ✗ |
| `(2,5)` | `setRangeText("Z",2,5)` | `2,3` | `2,3` ✓ boundary |
| `(3,7)` | `setRangeText("Z",2,5)` | `2,5` | `2,5` ✓ boundary |

⚠ **The two rows that agree are exactly the ones a test would be written from.** `x >= end` and
`x > end` differ only in the middle, so a case set drawn from boundary examples cannot see this. The
symptom: a caret *inside* the text being replaced comes back collapsed — type over your own
`@mention` suggestion and the widget loses the range it needs to replace next.

## 5. An inverted range collapses onto its END — but a single-edge setter wins instead

| call | from | result |
|---|---|---|
| `setSelectionRange(2, 1)` | — | `1,1` — the **end** wins |
| `setSelectionRange(5, 0)` | — | `0,0` — the **end** wins |
| `setSelectionRange(7, 1)` | value length 6 | `1,1` — clamp to 6, then collapse onto end |
| `el.selectionStart = 5` | `(1,3)` | `5,5` — the **start** wins, end dragged UP |
| `el.selectionEnd = 1` | `(2,4)` | `1,1` — the **end** wins, start dragged DOWN |

⭐ **The rule is not "the smaller edge wins" — it is "the edge you just SET wins, and the other is
dragged to it".** A single clamp shared by `setSelectionRange` and the two single-edge setters cannot
express that: it is three callers asking two questions. It is the same defect as §4, in the same file.

Concretely: the clamp owns the `setSelectionRange` resolution (collapse onto **end**), which is
already what `selectionEnd = n` needs; only `selectionStart = n` has to work *against* it, raising the
end itself before calling in. **The asymmetry is real, and a "symmetrical" guard on the other side is
inert** — a mutation removing it stays green, which is exactly how it was caught.

## 6. The selection store is per-document, and forgetting that is invisible

`document.createElement("textarea")` — brand new, empty, detached — reported `selectionStart === 6`.

A freshly created element cannot have a selection, so **the only way to read one is to read somebody
else's**. The store was a `HashMap<NodeId, _>` keyed by the bare node id that nothing ever cleared, so
a `NodeId` reused by the next document inherited the previous document's cursor.

The rule this engine already had, one field away in the same `thread_local!` block: **per-document
side tables are keyed `(arena, NodeId)` and cleared when a document is installed.** Its neighbour
`FRAME_CREATE_TRIED` carried the rule and a comment explaining it; the selection table, added later,
never joined. The clear now lives in `clear_document_side_tables()` — named for the class, not for one
table.

> ⭐ **A single-document gate is structurally blind to a per-document leak.** Everything in §1–§4 is
> gated and green in a fresh page; only the WPT suite, which reuses the runtime across files, could
> see this one. When a side table is keyed by node identity, the test that finds the bug has to load
> two documents.

## 7. `selectionDirection`: an invalid keyword RESETS, it does not stick

```js
el.selectionDirection = 'backward';   // "backward"
el.selectionDirection = 'sideways';   // the DEFAULT — not "backward"
```

⭐ **It takes a pair to see this.** Assigning `'sideways'` to a selection already at the default reads
back the default, which is consistent with *"invalid is ignored"* and with *"invalid resets"* alike.
Only assigning it after an explicit `'backward'` separates them. The setter is a total three-way map:
`backward` and `forward` map to themselves, **everything else maps to `none`**.

**The default direction is engine-split and WPT says so.** Chrome reports `"forward"` (its platform
has no directionless selection); Gecko and this engine report `"none"`. WPT asserts only
`assert_in_array(dir, ["forward", "none"])`, plus that assigning `"none"` gives the initial value back.

## Not built yet — the `select` event

Measured at tick 1392, deliberately left for its own tick. `select` fires **asynchronously** (never
synchronously from the setter), **bubbles**, is **not cancelable**, is `isTrusted`, fires on
**disconnected** nodes, and fires **only if the selection actually changed** — coalesced to at most one
queued task, so two identical changes in a row produce one event. `select-event.html` is the suite.

## Gates

* `engine/page/tests/g_selection_applicability.rs` — the 26 rows above.
* `engine/page/tests/g_text_selection.rs` — the original round-trip claims (tick 302).
* `engine/page/tests/g_set_range_text.rs` — splice behaviour.
* `engine/page/tests/g_textarea_value.rs` — the textarea value source. ⚠ Its `taLen` claim asserted
  `6` until tick 1392 and was **holding the engine to this bug**, with a comment reasoning the wrong
  value out. See [the journal for tick 1392](../loop/JOURNAL.md): a gate that argues for its expected
  value in prose is the one to re-measure first.
