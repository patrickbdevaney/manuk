# First layout: fallback fonts are interned lazily and system font bytes are mapped, not copied

> Landed PERF.1.3 (partial: see docs/sprint/PERF.md "PERF.1.3 results").

## Mechanism (engine/text/src/lib.rs)

- `FontContext::fallback_ids()` resolves the nine `FALLBACK_FAMILIES` to `fontdb::ID`s only (no file I/O).
  `resolve_face` interns them one at a time, in list order, and stops at the first face that covers the character - so an Arabic or
  CJK character no longer reads the colour-emoji face, three CJK collection faces, Symbols2, Hebrew, Devanagari and DejaVu first.
  `fallback_faces()` (all of them) is `#[cfg(test)]` only.
- `FaceData.data` is a `FaceBytes` (`Deref<Target=[u8]>`): `Source::File` is `memmap2`-mapped, `Binary`/`SharedFile` share the Arc.
  Before, every interned face was `to_vec()`-copied; a one-paragraph page's first layout paid ~45 of its 50 ms for that.
- Test `resolving_one_fallback_character_loads_only_the_faces_it_walks` goes red if `resolve_face` interns the whole list.

## Limits

First use of each distinct face still pays fontdue's eager outline parse (8-12 ms per Liberation face).
