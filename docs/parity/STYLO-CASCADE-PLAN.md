# Finishing the Stylo cascade (2A) — executable plan

The exact, source-verified path to replace `StyloEngine::cascade`'s delegation with a real
Stylo value cascade. All signatures verified against the on-disk `stylo-0.19.0` crate
(`~/.cargo/registry/src/index.crates.io-*/stylo-0.19.0/`). This is a **multi-session**
effort — the blocker is a 107-method DOM trait wall — so it is specified here in full so
execution needs no re-research.

## What already exists (do not redo)

- `stylo_probe.rs` — building a `Device` + `Stylist`, parsing author CSS with Stylo's
  parser, `append_stylesheet` + `flush`. **Proven.**
- `stylo_dom.rs` — `ElementDataStore` (NodeId → `ElementDataWrapper`), `StyloElement<'a>`
  handle, and a full **`selectors::Element` impl (30 methods)**; Stylo's real
  `matches_selector` runs over the arena DOM, tested end-to-end.
- `crate::Stylesheet` now retains its raw `source()` text so Stylo can re-parse it.

## The confirmed blocker: naming a `TElement`

Both cascade entry points are `where E: TElement`, even though the element is passed as
`None` and **no `TElement` method is called at runtime** (verified in
`properties/cascade.rs:284,314` — element is only touched via `map_or`/`map`). Rust still
requires a concrete `E` at the call site (turbofish). So a type implementing `TElement`
must exist. `TElement` (`dom.rs:409`) pulls in a closed graph of four mutually-referential
traits that must all be implemented (stubs are fine — methods `unimplemented!()` since
never called — but every signature and associated type must match exactly):

| trait | methods | supertraits / assoc types |
|---|---|---|
| `TDocument` | 5 | `ConcreteNode: TNode` |
| `TNode` | 20 | `Copy+Clone+Debug+PartialEq+NodeInfo`; `ConcreteElement`, `ConcreteDocument`, `ConcreteShadowRoot` |
| `TShadowRoot` | 6 | `Copy+Clone+Debug+PartialEq`; `ConcreteNode` |
| `TElement` | 76 | `Eq+PartialEq+Debug+Hash+Copy+Clone+SelectorsElement<Impl=SelectorImpl>+AttributeProvider`; `ConcreteNode`, `TraversalChildrenIterator` |

`StyloElement` already satisfies `SelectorsElement`; it still needs `Eq`, `Hash`, and an
`AttributeProvider` impl. **Decision:** implement the wall for the *real* `StyloElement`
(the Blitz approach) rather than a ZST stub — the real impl is reusable for incremental
restyle/invalidation later and is not much more than a correct stub, since our arena has
the needed navigation. Reference: DioxusLabs/blitz `blitz-dom/src/stylo_to_taffy` +
`blitz-dom`'s `TElement`/`TNode` impls.

## Step 1 — implement the DOM trait wall on `StyloElement`/`StyloNode`

Add a `StyloNode<'a>` handle (like `StyloElement` but for any node). Implement, in
`stylo_dom.rs`:
- `NodeInfo` for `StyloNode` (is_element / is_text).
- `TNode` for `StyloNode` (20): parent_node, first_child, last_child, prev/next_sibling,
  owner_doc, as_element, as_document, as_shadow_root, traversal children, opaque, etc.
- `TDocument` (5) + `TShadowRoot` (6) — mostly trivial over the arena.
- `AttributeProvider` for `StyloElement`.
- `TElement` (76) for `StyloElement` — the bulk. Many are `false`/`None`/no-op for a
  one-shot cascade (animation, snapshot, restyle-hint, pseudo, XBL/shadow bits). The
  load-bearing ones: `parent_element`, `traversal_children`, `is_html_element_in_html_document`,
  `style_attribute` (return the element's inline `style=` parsed to a
  `PropertyDeclarationBlock`), `borrow_data`/`mutate_data`/`has_data`/`ensure_data`
  (delegate to `ElementDataStore` — already built), `local_name`/`namespace`.

Derive `Eq`/`Hash` for `StyloElement` on `(node)` identity.

## Step 2 — match author rules (reuse existing matcher)

Keep our own parsed rule list; the `Stylist` does not expose `(selector, block)` for
re-matching. For each element, in tree order (parents first, for inheritance):

```rust
let guard = lock.read();
let mut winners: Vec<(u32 /*specificity*/, u32 /*source_order*/, Arc<Locked<PropertyDeclarationBlock>>)> = vec![];
for (order, rule) in sheet.rules(&guard).iter().enumerate() {          // stylesheet.rs:144
    if let CssRule::Style(style_rule) = rule {                          // stylesheets/mod.rs:334
        let sr = style_rule.read_with(&guard);                         // &StyleRule (style_rule.rs:26; fields pub)
        for sel in sr.selectors.slice() {
            if matches_selector(sel, 0, None, &stylo_element, &mut ctx) {   // our selectors::Element
                winners.push((sel.specificity(), order as u32, sr.block.clone()));
            }
        }
    }
}
winners.sort_by_key(|(spec, order, _)| (*spec, *order));                // ascending: later wins
```

Include the element's inline `style=` block last (highest priority) and the UA defaults
first (lowest). `!important` is handled by `PropertyDeclarationBlock` importance.

## Step 3 — cascade to `ComputedValues`

Cleanest entry (`stylist.rs:1983`, no rule tree):

```rust
pub fn compute_for_declarations<E: TElement>(
    &self, guards: &StylesheetGuards, parent_style: &ComputedValues,
    declarations: Arc<Locked<PropertyDeclarationBlock>>,
) -> Arc<ComputedValues>
```

Merge the winners into one block (push in ascending priority), then:

```rust
let guards = StylesheetGuards::same(&guard);
let parent = parent_cv.unwrap_or_else(|| stylist.device().default_computed_values());
let cv = stylist.compute_for_declarations::<StyloElement>(&guards, parent, merged_arc);
```

For correct origin/`!important`/`@layer` resolution instead of naive later-wins, use the
rule-tree route instead: build `ApplicableDeclarationBlock`s
(`from_declarations(block, CascadeLevel::same_tree_author_normal(), LayerOrder::root())`,
`applicable_declarations.rs:230`) → `stylist.rule_tree().compute_rule_node(&mut list, &guards)`
(`rule_tree/mod.rs:177`) → `properties::cascade::<StyloElement>(&stylist, None, &rule_node,
&guards, parent, layout_parent, FirstLineReparenting::No, &Default::default(), None,
Default::default(), RuleCascadeFlags::empty(), None, &mut Default::default(), None)`
(`properties/cascade.rs:66`).

## Step 4 — map `ComputedValues` → `crate::ComputedStyle`

Independently testable **now** against `stylist.device().default_computed_values()` (no
`TElement` needed). Accessors (`properties.mako.rs`): per-longhand `cv.clone_<ident>()`
(physical only) and per-struct `cv.get_<struct>()`. Reductions:

| field | accessor | reduce |
|---|---|---|
| display | `clone_display()` → packed `Display` | via `DisplayOutside`/`DisplayInside` (not a flat enum) |
| color | `clone_color()` → `AbsoluteColor` | `to_color_space(ColorSpace::Srgb)` → `components`/`alpha` × 255 |
| background-color | `clone_background_color()` → `Color` | `.resolve_to_absolute(&cv.clone_color())` then as above |
| font-size | `clone_font_size().computed_size().px()` | f32 |
| font-weight | `clone_font_weight().value()` | f32 → u16 |
| font-style | `clone_font_style()` | `== FontStyle::NORMAL` ? not italic |
| line-height | `clone_line_height()` → `GenericLineHeight` | Normal→size×1.2 / Number(n)→n×size / Length(l)→px |
| text-align | `clone_text_align()` → `TextAlign` (nested keyword) | match keyword |
| width/height/min/max | `clone_width()` → `Size`/`MaxSize` enum | `LengthPercentage(lp)`→Dim / `Auto`/`None` / content-keywords→Auto; handle anchor `_ =>` |
| margin-* | `clone_margin_top()` → `LengthPercentageOrAuto` | LengthPercentage→Dim / Auto |
| padding-* | `clone_padding_top()` → `NonNegativeLengthPercentage` | `.0` → Dim |
| border-*-width | `clone_border_top_width().0.to_f32_px()` | f32 (`.0` is `app_units::Au`) |
| top/right/bottom/left | `clone_top()` → `Inset` | LengthPercentage→Dim / Auto / anchor `_ =>` |
| position | `clone_position()` → `PositionProperty` | keyword |
| box-sizing | `clone_box_sizing()` → `BoxSizing` | keyword |
| z-index | `clone_z_index()` → `ZIndex` | Integer(i)/Auto |
| opacity | `clone_opacity()` → f32 | direct |
| overflow-x/y | `clone_overflow_x()` → `Overflow` | keyword |

`LengthPercentage` → `Dim`: `lp.to_length().map(|l| l.px())` → `Dim::Px`; else
`lp.to_percentage()` → `Dim::Percent`; else (mixed calc) `Dim::Calc`.

## Step 5 — wire + gate

`StyloEngine::cascade`: build lock/url/device/stylist once, parse each `sheet.source()`,
add UA defaults as a low-priority origin, then per element run steps 2-4. Keep
`MinimalCascade` as the default; `page` gains a `stylo` feature forwarding to
`manuk-css/stylo` and selects `StyloEngine` when enabled. Verify against the layout-parity
harness (should stay ≥ current 72/72) and add cascade unit tests (var(), @media via the
`Device`, child/attribute selectors) that the minimal engine can't pass.

## Version notes (0.19 vs neighbors)

`cascade` is `stylist`-first (no `device`/`quirks_mode` params — those come from the
stylist); gained `first_line_reparenting` + `try_tactic`. `white-space` is a shorthand
(longhands `white-space-collapse` + `text-wrap-mode`; no `clone_white_space()`).
`StyleSource` is declaration-block-only (`from_declarations`). `AbsoluteColor` replaced
`RGBA` (no `.to_rgba()`; go through sRGB). `Size`/`Inset` have anchor-positioning variants
— match arms need `_ =>`.
