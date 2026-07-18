# MEDIA PIPELINE — MSE, the attachment handshake, and the decode registry

> The watch-the-web track (M1…M7). This file records the mechanisms that are easy to get subtly
> wrong and expensive to rediscover. M1 (the MSE byte pipe) is landed; M3–M5 (demux/decode) are not.

## Adaptive streaming never touches `<video src="file.mp4">`

Every site that matters for watching — YouTube, Netflix, Twitch, Vimeo — and every player library
(hls.js, dash.js, shaka, video.js) does the same thing instead: construct a `MediaSource`, hand the
element a `blob:` URL for it, wait for `sourceopen`, `addSourceBuffer(mime)`, then `appendBuffer()`
media segments fetched over XHR in a loop clocked by `updateend`.

The consequence for an engine that lacks MSE is worse than "video does not play". Those players run
`new MediaSource()` inside a capability probe at **module-evaluation** time, so a missing name is a
`ReferenceError` that kills the player script before it renders a single control — and takes the
surrounding bundle's evaluation with it. A player that cannot construct its source object cannot
fall back to progressive download either. It just stops.

## The attachment handshake is the ONLY channel from element to MediaSource

`video.src = URL.createObjectURL(mediaSource)` is the single moment the element learns which
`MediaSource` it is playing. There is nothing else — no attribute, no registry lookup by name.

That makes `URL.createObjectURL` load-bearing rather than a convenience: it registers the object
against the returned `blob:` id, and the `src` **setter** must intercept the assignment, resolve the
id back to the object, and flip the source `closed` → `open`. Miss the interception and the object
URL is stored as an ordinary attribute string, the source stays `closed`, `sourceopen` never fires,
and the player waits on that event **forever** — a hang with nothing wrong in the DOM to see. That
is exactly the failure `g_mse`'s `syncopen:` claim is positioned to catch.

Two details that bite:

- **Revocation must not detach.** Players call `URL.revokeObjectURL(url)` immediately after
  assigning `src`. Once attached, the element holds the reference; revoking only removes the id from
  the registry. Tearing the stream down there kills it at the instant it starts.
- **Look the reflected `src` accessor up lazily, on the prototype chain, at call time.** Reflection
  installs `src` on the prototype and the interception is installed per element; resolving it
  eagerly bakes in an install-order assumption that is not ours to rely on.

`srcObject = mediaSource` is the newer form of the same handshake with no object URL in between, and
needs its own interception.

## The decode registry: a `false` is worth more than an unbacked `true`

`MediaSource.isTypeSupported()` answers from `__mseCodecs`, the registry of what something
downstream can **genuinely** decode. It is empty until a real decoder lands, so every answer is
`false` and every player takes its documented fallback path.

This is the opposite of the instinct to stub `true`. A `true` that is not backed by a decoder steers
the player onto the adaptive path, where it appends segments and then polls `buffered` for a range
that can never grow — hanging forever, with the failure surfacing far from its cause. The honest
`false` fails immediately, legibly, and in a branch the player already handles.

The registry is the hand-off seam for the rest of the track: M3 (demux) and M4/M5 (AAC / VP9 decode)
push the types they land, and `isTypeSupported` starts saying yes for exactly what can be played,
with no change to any of the surrounding machinery. `g_mse` asserts the `false` **first** and then
registers a codec and asserts the answer flips — which is what proves the honesty is a seam and not
a hardcoded constant.

## `buffered` is a media timeline, not a byte count

Bytes appended are held in order (that queue is what a demuxer will read), but nothing is demuxed,
so no media timeline exists and `buffered` is honestly empty. Reporting a fabricated range from
byte counts would be the same unbacked-`true` mistake in a different place.

## The append sequence is a task sequence, and that is load-bearing

`updatestart` fires synchronously inside `appendBuffer`; `update` and `updateend` fire on a **later
task** (`setTimeout`, not a microtask). A player that re-enters `appendBuffer` from its own
`updateend` must find `updating` already `false` and the previous task fully unwound — a microtask
would not guarantee that. Getting this wrong makes the steady-state fill loop throw on its second
segment.

The spec's `InvalidStateError` on a re-entrant `appendBuffer` (and on `endOfStream()` while a buffer
is updating) exists so the player can queue instead of corrupting the stream; players branch on that
exact error name.

## An `updateend` handler that appends is re-entered by its own append

Worth stating separately, because it cost a tick. A listener that appends a segment is re-invoked by
the very append it just made — an unbounded append/timer chain that never lets the event loop drain.
The page does not fail; it **hangs**, at full CPU, indistinguishable from a slow build. `g_mse`
therefore drives its sequence from one listener dispatching on a step counter, never from a listener
that registers another listener.

This is a real player's fill-loop shape, so the same hazard exists in page content. Note the engine
consequence: a runaway timer chain in page script currently spins `Page::load` without a bound.

## Gates: flush the record on every push, not at the end

`g_mse` writes its result to the DOM on **every** push. The sequence is asynchronous and only
completes on `sourceended`, so a single end-of-run write means any earlier break — source never
opened, append threw, listener never fired — reports the identical empty `-`, naming no claim and
pointing at no mechanism. Per-push flushing makes the last recorded claim the failure's location.
This was not hypothetical: the first RED probe reported `got: -` and proved nothing until fixed.
