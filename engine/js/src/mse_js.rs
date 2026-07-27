//! **MSE — `MediaSource` / `SourceBuffer`: the byte pipe, built before the decoder.**
//!
//! Adaptive streaming is not `<video src>`. Every site that matters for watching — YouTube,
//! Netflix, Twitch, Vimeo, and every player library (hls.js, dash.js, shaka, video.js) — does the
//! same thing instead: construct a `MediaSource`, hand the element a `blob:` URL for it, wait for
//! `sourceopen`, `addSourceBuffer(mime)`, then `appendBuffer()` media segments fetched over XHR in
//! a loop driven by `updateend`. The element's `src` is never a media file.
//!
//! **What was broken.** None of those names existed. `new MediaSource()` was a `ReferenceError`,
//! which every one of those players throws at *module-evaluation* time inside its capability probe —
//! so the failure was not "video does not play", it was **the player script dies before it renders a
//! single control**, taking the surrounding page bundle with it. A player that cannot even construct
//! its source object cannot fall back to progressive download either; it just stops.
//!
//! **What this tick builds, and what it deliberately does not.** The whole object graph and state
//! machine: `MediaSource` (readyState / duration / `sourceopen`,`sourceended`,`sourceclose`),
//! `SourceBuffer` (`appendBuffer` accepting and queueing real bytes, the `updating` flag, and the
//! `updatestart`→`update`→`updateend` task sequence that drives every append loop),
//! `SourceBufferList`, `TimeRanges`, `URL.createObjectURL`/`revokeObjectURL`, and the attachment
//! handshake that flips a `<video>` over to a MediaSource when its `src` is set to an object URL.
//!
//! **Since M3 (tick 234) the bytes are read.** `__demux` hands the accumulated stream to
//! `manuk-media` and populates `buffered`, `videoTracks`/`audioTracks` and the source's `duration`
//! from the container itself — so an adaptive player's `updateend` loop can finally steer, which is
//! what it reads `buffered` for. Gated by `g_media_buffered`.
//!
//! **There is still no decoder, and this file does not pretend otherwise.** Knowing *where* the
//! H.264 is and being able to decode it are different claims. No frame is produced. That honesty is
//! load-bearing in exactly one place: `__mseCodecs` is the registry of MIME types the decode layer
//! can *actually* handle, it is **empty today**, and `MediaSource.isTypeSupported()` answers from
//! it. So every player asks "can you do VP9?", is told **no**, and takes its documented fallback
//! path — instead of being told yes and then stalling forever on a `buffered` range whose media
//! never decodes, which is the strictly worse outcome and the one a stub would have produced.
//!
//! That registry is the hand-off point for the rest of the media track: M4/M5 (AAC / VP9 decode)
//! populate `__mseCodecs`, and `isTypeSupported` starts saying yes for exactly what can be played,
//! with no change to any of the machinery below.

/// The MSE surface. Evaluated after the main prelude (so `setTimeout`, `DOMException` and the inert
/// sweep have all run) and after `dom_bindings`' `install` (so `URL` exists to hang
/// `createObjectURL` on).
pub const MSE_JS: &str = r#"
(function () {
  'use strict';
  var g = globalThis;

  // ── The decode registry — the one place this file is allowed to claim a capability.
  //
  // A MIME/codecs string is "supported" only when something downstream can genuinely decode it.
  // Nothing can, yet, so this is empty and every `isTypeSupported` answer is `false`. M3/M4/M5 push
  // the types they land here; nothing else in this file changes when they do.
  if (!g.__mseCodecs) { g.__mseCodecs = []; }

  // ── WebM codec acceptance, in ONE place — the tick-634 rule.
  //
  // Two callers have to answer "does this WebM's codec list name something we decode":
  // `isTypeSupported` just below, and `HTMLMediaElement.canPlayType` over in event_loop.rs. If
  // they answer it with two regexes they will disagree the first time either is edited, and the
  // disagreement surfaces as a page being told it can play something it cannot. That is the
  // one-rule-N-implementations defect this project has caught in eight consecutive ticks, so the
  // rule lives here once and both read it.
  //
  // **Accepts iff EVERY named codec is `av01.*`.** AV1 is what `re_rav1d` has decoded since
  // t354, and t633's EBML reader now supplies its samples — the container was never part of the
  // decode question, which is exactly why answering `false` for AV1-in-WebM was a false absence
  // rather than a conservative choice.
  //
  // The prefix **includes the dot**, and that is not cosmetic: it matches
  // `manuk_media::av1::can_decode` character for character. A WebM AV1 track whose `CodecPrivate`
  // is not a readable `av1C` reports the bare string `av01`, and the Rust side refuses that one.
  // Both sides must compute the SAME QUANTITY or the claim is about a different thing than the
  // capability behind it.
  //
  // `vp9`/`vp8`/`opus`/`vorbis` — the short forms a WebM `codecs=` parameter actually carries,
  // per `webm::codec_string` — are all NO, and a MIXED list (`av01.…,opus`) is NO for the same
  // reason: rendering the video and silently dropping the audio is not playing the file.
  g.__manukWebmCodecsDecodable = function (codecs) {
    if (typeof codecs !== 'string') { return false; }
    var list = codecs.replace(/^"|"$/g, '').split(',');
    if (list.length === 0) { return false; }
    for (var i = 0; i < list.length; i++) {
      if (!/^av01\.[0-9a-z.]+$/.test(list[i].trim().toLowerCase())) { return false; }
    }
    return true;
  };

  var canDecode = function (type) {
    if (typeof type !== 'string' || type === '') { return false; }
    var want = type.toLowerCase().replace(/\s+/g, '');
    for (var i = 0; i < g.__mseCodecs.length; i++) {
      if (String(g.__mseCodecs[i]).toLowerCase().replace(/\s+/g, '') === want) { return true; }
    }
    // ── The built-in truth (tick 349): what the tree GENUINELY plays end-to-end, no registry
    // push required. MP4 only — `manuk_media::demux` opens (f)MP4, `H264Decoder` decodes
    // Baseline-profile H.264 (`avc1.42……` — the profile byte is the pair after "avc1.", 0x42;
    // High/Main are refused exactly as `video::can_decode` refuses them), and the AAC path
    // (`mp4a.40.*`) demuxes+decodes to PCM (G_MEDIA_AAC), and AV1-in-MP4 (`av01.*`) decodes via
    // re_rav1d in the shell lane (tick 354).
    //
    // ── WebM. The container line was drawn at t633; t634 moved the AV1 codec line with it.
    //
    // `manuk_media::webm` opens EBML: tracks, codec strings, a verified sample table and
    // `buffered`. The **bare** container form means for WebM what it has always meant for MP4 on
    // the line below — *we can open this container* — which is `isTypeSupported`'s documented
    // contract for a type with no codecs parameter, is what Chrome answers, and is what makes the
    // demuxer reachable from a page at all (`addSourceBuffer` is the only door to `__demux`, and
    // it consults this function).
    //
    // The **codecs=** form was `false` for everything at t633, on the stated ground that no VP9
    // and no Opus decoder exists in this tree. True, and it was not the whole truth: AV1 has
    // decoded here since t354 and is the other codec WebM carries — so `codecs="av01.…"` is now
    // yes, on the evidence of `G_MEDIA_WEBM_AV1` decoding real EBML samples to real pictures, and
    // everything else stays no. Saying yes to `codecs="vp9"` would still be precisely the
    // black-rectangle failure MEDIA.md warns about.
    //
    // Two things deliberately do NOT move, and both are load-bearing:
    //   * `HTMLMediaElement.canPlayType` still answers `''` for **bare** `video/webm`
    //     (event_loop.rs). If it said otherwise, a `<video>` with an unqualified `.webm` <source>
    //     before its `.mp4` one would select a file that is overwhelmingly likely to be VP9+Opus
    //     over the MP4 we can decode — a REGRESSION traded for a capability, which the ratchet
    //     refuses. A <source> that NAMES av01 carries no such risk and moves to 'probably'.
    //   * every real adaptive player (hls.js, dash.js, shaka) probes WITH codecs, so none of them
    //     is steered by the bare form. It is feature-detection code that reads it.
    var wm = /^(video|audio)\/webm($|;)/.exec(want);
    if (wm) {
      var wq = want.indexOf('codecs=');
      if (wq < 0) { return true; }
      return g.__manukWebmCodecsDecodable(want.slice(wq + 7));
    }
    var m = /^(video|audio)\/mp4($|;codecs=)/.exec(want);
    if (!m) { return false; }
    var q = want.indexOf('codecs=');
    if (q < 0) { return true; } // bare container: we can open MP4, per isTypeSupported's contract
    var list = want.slice(q + 7).replace(/^"|"$/g, '').split(',');
    for (var j = 0; j < list.length; j++) {
      var c = list[j];
      if (c === '') { return false; }
      if (/^avc1\.42[0-9a-f]{4}$/.test(c)) { continue; }   // H.264 Baseline only
      if (/^mp4a\.40(\.\d+)?$/.test(c)) { continue; }       // AAC
      if (/^av01\./.test(c)) { continue; }                  // AV1 (re_rav1d, tick 354)
      return false;
    }
    return true;
  };

  // ── The decode question has THREE askers, and exactly one answer (tick 635).
  //
  // `MediaSource.isTypeSupported` (below), `HTMLMediaElement.canPlayType` (event_loop.rs) and
  // `navigator.mediaCapabilities.decodingInfo` (further down this file) all ask *"can this tree
  // decode this contentType"*. t634 consolidated the first two after the WebM answers drifted;
  // adding `decodingInfo` with its own regex would have restored the defect at full size, one
  // tick after paying to remove it. So `canDecode` is published once and all three read it.
  //
  // Published under a `__manuk` name rather than exported, because the page must not be able to
  // see or patch the thing three spec surfaces agree through.
  g.__manukCanDecodeType = function (t) { return canDecode(t); };

  var fail = function (msg, name) { return new g.DOMException(msg, name); };

  // ── TimeRanges. Immutable, index-checked, and empty until a demuxer says otherwise.
  function TimeRanges(ranges) {
    var r = ranges || [];
    Object.defineProperty(this, 'length', { get: function () { return r.length; } });
    this.start = function (i) {
      if (i >>> 0 !== i || i >= r.length) { throw fail('index out of range', 'IndexSizeError'); }
      return r[i][0];
    };
    this.end = function (i) {
      if (i >>> 0 !== i || i >= r.length) { throw fail('index out of range', 'IndexSizeError'); }
      return r[i][1];
    };
  }
  g.TimeRanges = TimeRanges;

  // ── The listener mixin. Every non-DOM platform object here hand-rolls these four, because
  // `EventTarget.prototype` in this engine is the DOM chain's, not a general one (see the `iface`
  // predicate in the prelude). Matches the WebSocket/EventSource shape exactly: the `on…` handler
  // runs before the listener list, the list is copied before iteration because a listener may
  // remove itself, and every callback is contained — one throwing listener must not eat the rest
  // of an append loop.
  var target = function (proto) {
    proto.addEventListener = function (t, fn) {
      if (typeof fn === 'function') { (this.__ls[t] = this.__ls[t] || []).push(fn); }
    };
    proto.removeEventListener = function (t, fn) {
      var a = this.__ls[t]; if (!a) { return; }
      var i = a.indexOf(fn); if (i >= 0) { a.splice(i, 1); }
    };
    proto.dispatchEvent = function (ev) { this.__fire(ev && ev.type, ev); return true; };
    proto.__fire = function (type, ev) {
      ev = ev || { type: type, target: this };
      var on = this['on' + type];
      if (typeof on === 'function') { try { on.call(this, ev); } catch (e) {} }
      var a = (this.__ls[type] || []).slice();
      for (var i = 0; i < a.length; i++) { try { a[i].call(this, ev); } catch (e) {} }
    };
    // Spec-shaped: these are *tasks*, not microtasks. An append loop that re-enters
    // `appendBuffer` from its own `updateend` must find `updating` already false and the previous
    // task fully unwound, which a microtask would not guarantee.
    proto.__fireLater = function (type) {
      var self = this;
      g.setTimeout(function () { self.__fire(type); }, 0);
    };
  };

  // ── SourceBufferList. Array-indexed, because players write `ms.sourceBuffers[0]`.
  function SourceBufferList() {
    this.__ls = {};
    this.__items = [];
    Object.defineProperty(this, 'length', { get: function () { return this.__items.length; } });
  }
  target(SourceBufferList.prototype);
  SourceBufferList.prototype.__sync = function () {
    // Re-index as own properties so `list[0]` works without a Proxy.
    var i = 0;
    while (Object.prototype.hasOwnProperty.call(this, i)) { delete this[i]; i++; }
    for (i = 0; i < this.__items.length; i++) { this[i] = this.__items[i]; }
  };
  g.SourceBufferList = SourceBufferList;

  // ── SourceBuffer. The append pipe.
  function SourceBuffer(parent, type) {
    this.__ls = {};
    this.__parent = parent;
    this.__type = type;
    this.__updating = false;
    // The appended segments, held in order — and, since M3, actually read. `__bin` is the same
    // bytes in the one-char-per-byte form the Rust boundary takes, accumulated as they arrive
    // rather than rebuilt per append: the demuxer needs the *whole* stream (an init segment
    // defines the tracks that every later media segment's samples belong to), so re-concatenating
    // the chunk list on every append would make an N-segment stream O(N²) in exactly the case that
    // matters — a long video, appended segment by segment, for an hour.
    this.__chunks = [];
    this.__bin = '';
    this.__bytes = 0;
    this.__ranges = [];
    this.mode = 'segments';
    this.timestampOffset = 0;
    this.appendWindowStart = 0;
    this.appendWindowEnd = Infinity;
    this.audioTracks = []; this.videoTracks = []; this.textTracks = [];
    var self = this;
    Object.defineProperty(this, 'updating', { get: function () { return self.__updating; } });
    // The demuxed presentation timeline (M3). Empty until something has been appended AND parsed —
    // a player reading an empty one sees "you have nothing buffered", which stays true rather than
    // becoming a comfortable lie the moment a demuxer exists.
    Object.defineProperty(this, 'buffered', {
      get: function () {
        if (self.__parent === null) { throw fail('the source buffer has been removed', 'InvalidStateError'); }
        return new TimeRanges(self.__ranges || []);
      }
    });
  }
  target(SourceBuffer.prototype);

  // The two checks that guard every mutating method, in the spec's order.
  SourceBuffer.prototype.__guard = function () {
    if (this.__parent === null) { throw fail('the source buffer has been removed from its MediaSource', 'InvalidStateError'); }
    if (this.__updating) { throw fail('a previous operation on this SourceBuffer is still in progress', 'InvalidStateError'); }
  };

  SourceBuffer.prototype.appendBuffer = function (data) {
    this.__guard();
    var bytes = null;
    if (data instanceof g.ArrayBuffer) { bytes = new Uint8Array(data.slice(0)); }
    else if (data && data.buffer instanceof g.ArrayBuffer && typeof data.byteLength === 'number') {
      bytes = new Uint8Array(data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength));
    } else {
      throw new TypeError('appendBuffer expects an ArrayBuffer or ArrayBufferView');
    }
    var ms = this.__parent;
    if (ms.readyState === 'closed') { throw fail('the MediaSource is closed', 'InvalidStateError'); }
    // An append to an ended stream re-opens it — this is how a live player resumes after
    // `endOfStream()`, and skipping it strands the stream permanently.
    if (ms.readyState === 'ended') { ms.__setReadyState('open'); }

    this.__chunks.push(bytes);
    this.__bytes += bytes.byteLength;
    var s = '';
    for (var i = 0; i < bytes.length; i++) { s += String.fromCharCode(bytes[i]); }
    this.__bin += s;
    this.__updating = true;
    this.__fire('updatestart');
    var self = this;
    // The append completes on a later task, exactly as it does when a real demuxer is doing the
    // work — and since M3 a real demuxer *is* doing the work, on this task, which is why the
    // asynchrony was built this way in the first place.
    g.setTimeout(function () {
      self.__demux();
      self.__updating = false;
      self.__fire('update');
      self.__fire('updateend');
    }, 0);
  };

  // ── M3: read what was appended.
  //
  // **Failure here is silent by design, and that is not the same as ignored.** An MSE append is
  // incremental: a player hands over an init segment that defines tracks but contains no media,
  // then media segments that contain no track definitions, and either can arrive split across
  // several `appendBuffer` calls. "I cannot parse this *yet*" is therefore the ordinary state of a
  // healthy stream, not an error — so a failed demux leaves the previous ranges standing and waits
  // for more bytes. Throwing, or clearing `buffered`, would break every player on its first
  // partial append.
  //
  // What a demux failure must never do is *invent* a timeline, which is the failure mode MEDIA.md
  // names: a player told it has buffered media it does not have stalls forever waiting for a frame
  // that never decodes. Empty is honest; wrong is fatal.
  SourceBuffer.prototype.__demux = function () {
    if (typeof g.__mseDemux !== 'function') { return; }
    var info;
    try { info = JSON.parse(g.__mseDemux(this.__bin)); } catch (e) { return; }
    if (!info || !info.ok) { return; }
    this.__ranges = info.ranges || [];
    this.__info = info;
    // The track lists a player reads to decide what it is about to play. Populated from the
    // container, so an audio-only or video-only stream reports itself as one — which is how an
    // adaptive player knows it still needs to open the other SourceBuffer.
    var vt = [], at = [];
    for (var i = 0; i < (info.tracks || []).length; i++) {
      var t = info.tracks[i];
      var entry = { id: String(t.id), kind: t.kind, codec: t.codec, language: '', label: '' };
      if (t.kind === 'video') { entry.width = t.width; entry.height = t.height; vt.push(entry); }
      else if (t.kind === 'audio') { entry.channels = t.channels; entry.sampleRate = t.sampleRate; at.push(entry); }
    }
    this.videoTracks = vt;
    this.audioTracks = at;
    // `MediaSource.duration` is NaN until something knows better. A demuxed `moov` knows better —
    // but only when it actually carries a duration: a bare media segment reports 0, and writing
    // that over a known duration would truncate the timeline the player is seeking within.
    var ms = this.__parent;
    if (ms && info.duration > 0 && !(ms.__duration > 0)) {
      ms.__duration = info.duration;
      ms.__fireDurationChange();   // a demuxed moov just gave the timeline its length
    }
    // ── The playback JOIN (tick 349). This SourceBuffer's accumulated stream is the ONLY copy of
    // the media — the element's src is a blob: URL no fetch can serve — so every settled append
    // that demuxed a video track hands the FULL stream to the host, which decodes it and drives
    // frames into the page exactly as it does for a progressive <video src>. Video-track buffers
    // only: an audio-only SourceBuffer has no frames for the host's video drive, and publishing
    // it would overwrite the video stream under the same node.
    if (typeof g.__msePublish === 'function' && ms && ms.__element && ms.__element.__nodeId != null) {
      var hasVideo = false;
      for (var v = 0; v < (info.tracks || []).length; v++) {
        if (info.tracks[v].kind === 'video') { hasVideo = true; break; }
      }
      if (hasVideo && this.__bin.length > 0) {
        g.__msePublish(String(ms.__element.__nodeId), this.__bin);
      }
    }
  };

  SourceBuffer.prototype.abort = function () {
    if (this.__parent === null) { throw fail('the source buffer has been removed', 'InvalidStateError'); }
    if (this.__parent.readyState !== 'open') { throw fail('the MediaSource is not open', 'InvalidStateError'); }
    if (this.__updating) {
      this.__updating = false;
      this.__fire('abort');
      this.__fire('updateend');
    }
    this.appendWindowStart = 0;
    this.appendWindowEnd = Infinity;
  };

  SourceBuffer.prototype.remove = function (start, end) {
    this.__guard();
    start = Number(start); end = Number(end);
    if (!(start >= 0) || !(end > start)) { throw new TypeError('remove() needs 0 <= start < end'); }
    if (this.__parent.readyState !== 'open') { throw fail('the MediaSource is not open', 'InvalidStateError'); }
    this.__updating = true;
    this.__fire('updatestart');
    var self = this;
    g.setTimeout(function () {
      self.__updating = false;
      self.__fire('update');
      self.__fire('updateend');
    }, 0);
  };

  SourceBuffer.prototype.changeType = function (type) {
    this.__guard();
    if (!canDecode(type)) { throw fail('unsupported type: ' + type, 'NotSupportedError'); }
    this.__type = type;
  };
  g.SourceBuffer = SourceBuffer;

  // ── MediaSource.
  function MediaSource() {
    this.__ls = {};
    this.__readyState = 'closed';
    this.__duration = NaN;
    this.__element = null;
    this.sourceBuffers = new SourceBufferList();
    this.activeSourceBuffers = new SourceBufferList();
    var self = this;
    Object.defineProperty(this, 'readyState', { get: function () { return self.__readyState; } });
    Object.defineProperty(this, 'duration', {
      get: function () { return self.__readyState === 'closed' ? NaN : self.__duration; },
      set: function (v) {
        v = Number(v);
        if (v < 0 || v !== v) { throw new TypeError('duration must be a non-negative number'); }
        if (self.__readyState !== 'open') { throw fail('the MediaSource is not open', 'InvalidStateError'); }
        for (var i = 0; i < self.sourceBuffers.__items.length; i++) {
          if (self.sourceBuffers.__items[i].updating) {
            throw fail('a SourceBuffer is still updating', 'InvalidStateError');
          }
        }
        var old = self.__duration;
        self.__duration = v;
        if (v !== old) { self.__fireDurationChange(); }   // NaN!==number is true, so first set fires
      }
    });
  }
  target(MediaSource.prototype);

  // `durationchange` on the ELEMENT, the moment the timeline length becomes known — the event a
  // player binds to size its scrub bar and enable seeking. Fired from BOTH the `duration` setter
  // (the explicit API) and the demux path (a `moov` that carries a duration), so however the length
  // arrives, the page hears it once. No element yet (an unattached MediaSource) = no one to tell.
  MediaSource.prototype.__fireDurationChange = function () {
    var el = this.__element;
    if (el && el.dispatchEvent) {
      try { el.dispatchEvent(new g.Event('durationchange')); } catch (e) {}
    }
  };

  MediaSource.prototype.__setReadyState = function (state) {
    if (this.__readyState === state) { return; }
    this.__readyState = state;
    var evt = state === 'open' ? 'sourceopen' : (state === 'ended' ? 'sourceended' : 'sourceclose');
    this.__fireLater(evt);
  };

  MediaSource.prototype.addSourceBuffer = function (type) {
    // The spec's exact order — a player distinguishes these three, and picking the wrong one sends
    // it down the wrong recovery branch.
    if (type === undefined || type === null || String(type) === '') {
      throw new TypeError('addSourceBuffer requires a non-empty type');
    }
    if (!canDecode(String(type))) {
      throw fail('unsupported MIME type or codec: ' + type, 'NotSupportedError');
    }
    if (this.__readyState !== 'open') {
      throw fail('the MediaSource is not open', 'InvalidStateError');
    }
    var sb = new SourceBuffer(this, String(type));
    this.sourceBuffers.__items.push(sb);
    this.sourceBuffers.__sync();
    this.activeSourceBuffers.__items.push(sb);
    this.activeSourceBuffers.__sync();
    this.sourceBuffers.__fire('addsourcebuffer');
    return sb;
  };

  MediaSource.prototype.removeSourceBuffer = function (sb) {
    var i = this.sourceBuffers.__items.indexOf(sb);
    if (i < 0) { throw fail('that SourceBuffer is not attached to this MediaSource', 'NotFoundError'); }
    this.sourceBuffers.__items.splice(i, 1);
    this.sourceBuffers.__sync();
    var j = this.activeSourceBuffers.__items.indexOf(sb);
    if (j >= 0) { this.activeSourceBuffers.__items.splice(j, 1); this.activeSourceBuffers.__sync(); }
    sb.__parent = null;
    this.sourceBuffers.__fire('removesourcebuffer');
  };

  MediaSource.prototype.endOfStream = function (error) {
    if (this.__readyState !== 'open') { throw fail('the MediaSource is not open', 'InvalidStateError'); }
    for (var i = 0; i < this.sourceBuffers.__items.length; i++) {
      if (this.sourceBuffers.__items[i].updating) {
        throw fail('a SourceBuffer is still updating', 'InvalidStateError');
      }
    }
    this.__endOfStreamError = error || '';
    this.__setReadyState('ended');
  };

  MediaSource.prototype.setLiveSeekableRange = function (start, end) {
    if (this.__readyState !== 'open') { throw fail('the MediaSource is not open', 'InvalidStateError'); }
    this.__liveSeekable = [[Number(start), Number(end)]];
  };
  MediaSource.prototype.clearLiveSeekableRange = function () {
    if (this.__readyState !== 'open') { throw fail('the MediaSource is not open', 'InvalidStateError'); }
    this.__liveSeekable = [];
  };

  // The capability question every player asks first. It answers from the decode registry, so it is
  // `false` for everything until something can genuinely play it. See the module doc: a `true` here
  // that is not backed by a decoder is worse than a `false`, because it steers the player onto a
  // path that then hangs instead of onto its fallback.
  MediaSource.isTypeSupported = function (type) { return canDecode(type); };
  MediaSource.__manuk = true;
  g.MediaSource = MediaSource;

  // ── Object URLs. `URL` is already installed by `dom_bindings` at this point.
  //
  // The MSE attachment handshake is `video.src = URL.createObjectURL(mediaSource)`, so this
  // registry is not a convenience — it is the only channel by which the element ever learns which
  // MediaSource it is playing.
  if (typeof g.URL === 'function') {
    var blobs = Object.create(null);
    var seq = 0;
    g.URL.createObjectURL = function (obj) {
      if (obj === undefined || obj === null) { throw new TypeError('createObjectURL requires an object'); }
      var origin = (g.location && g.location.origin) ? g.location.origin : 'null';
      var id = 'blob:' + origin + '/' + 'manuk-' + (++seq) + '-' +
               ((seq * 2654435761) % 4294967296).toString(16);
      blobs[id] = obj;
      return id;
    };
    g.URL.revokeObjectURL = function (url) {
      var id = String(url);
      var obj = blobs[id];
      // Revoking the URL of an *attached* MediaSource does not close it — the element holds the
      // reference now. Players revoke immediately after assigning `src`, so getting this wrong
      // tears down the stream at the exact moment it starts.
      delete blobs[id];
      if (obj === undefined) { return; }
    };
    g.__mseLookup = function (url) { return blobs[String(url)]; };
  }

  // ── The attachment handshake, called by `__manukMedia`'s `src` setter.
  //
  // Returns true when `url` named a MediaSource and the element took it. That is what flips the
  // source from 'closed' to 'open' and fires `sourceopen` — the event every player waits for before
  // it will call `addSourceBuffer`.
  g.__mseAttach = function (el, url) {
    var obj = g.__mseLookup ? g.__mseLookup(url) : undefined;
    if (!(obj instanceof MediaSource)) {
      // Switching away from a MediaSource detaches it, and a detached source is closed.
      if (el.__ms) { var old = el.__ms; el.__ms = null; old.__element = null; old.__setReadyState('closed'); }
      return false;
    }
    if (obj.__element && obj.__element !== el) { return false; }
    el.__ms = obj;
    obj.__element = el;
    obj.__setReadyState('open');
    return true;
  };

  // ══ navigator.mediaCapabilities (tick 635) — how a modern player picks a rendition ═══════════
  //
  // Shaka, dash.js, hls.js and YouTube's own player all call `decodingInfo()` on boot, once per
  // candidate rendition, and drop the ones it calls unsupported. Before this it was `undefined`,
  // so the call was a **TypeError** — not a missing nicety but the throw-class that blanks a page,
  // since a player that throws while enumerating renditions never gets to render any of them.
  //
  // **`supported` is the whole load-bearing field**, and it is the same answer `isTypeSupported`
  // gives because it is literally the same function. The other two are ranking hints, and the
  // honest values here are not the flattering ones:
  //
  //   * `powerEfficient: false` — **factually true of this tree**: every decoder here is software
  //     (openh264, symphonia, re_rav1d) and there is no VA-API/VideoToolbox/DXVA path at all. A
  //     grep for a hardware decoder finds nothing, which is what makes this a checkable claim
  //     rather than a modest-sounding guess — and what makes it a lie the day one lands.
  //   * `smooth: supported` — we do NOT model decode throughput, so this cannot honestly
  //     discriminate 4K from 360p, and it says so here rather than pretending to. It matches what
  //     Chrome answers for `type:'file'` on a software-decode desktop, and players treat `smooth`
  //     as a preference input rather than a filter (shaka gates variants on `supported`;
  //     `preferredDecodingAttributes` is empty by default). **If a player is ever observed
  //     excluding renditions on it, this becomes a measurement tick, not a constant to re-tune.**
  //
  // `webrtc` answers `supported: false` because WebRTC is explicitly out of scope (STATUS.md),
  // which is an honest no about a decided non-goal rather than an absence nobody has looked at.
  function mcInvalid(msg) { return g.Promise.reject(new g.TypeError(msg)); }

  var mediaCapabilities = {
    decodingInfo: function (config) {
      // The spec's validation, and it runs BEFORE anything else: a bad config REJECTS, it does not
      // resolve `supported:false`. A player distinguishes "you told me no" from "you did not
      // understand the question", and collapsing the two hides its own bugs.
      if (config === null || typeof config !== 'object') {
        return mcInvalid('decodingInfo requires a MediaDecodingConfiguration');
      }
      var t = config.type;
      if (t !== 'file' && t !== 'media-source' && t !== 'webrtc') {
        return mcInvalid("decodingInfo: type must be 'file', 'media-source' or 'webrtc'");
      }
      if (!config.audio && !config.video) {
        return mcInvalid('decodingInfo: at least one of audio or video is required');
      }
      var ok = true;
      var parts = [config.video, config.audio];
      for (var i = 0; i < parts.length; i++) {
        var p = parts[i];
        if (!p) { continue; }
        if (typeof p.contentType !== 'string' || p.contentType === '') {
          return mcInvalid('decodingInfo: contentType is required and must be a string');
        }
        // WebRTC is a decided non-goal; every other transport routes to the ONE decode answer.
        if (t === 'webrtc' || !g.__manukCanDecodeType(p.contentType)) { ok = false; }
      }
      return g.Promise.resolve({
        supported: ok,
        smooth: ok,
        powerEfficient: false,
        // The spec echoes the input back so a player can correlate an answer with the rendition
        // it asked about — several drive their variant filter off exactly this.
        configuration: config,
      });
    },
    // Encoding is a recorder's question (MediaRecorder), and nothing here encodes. It is present
    // and answers a truthful no, because `typeof …encodingInfo === 'function'` is what a feature
    // detect reads, and an absent method is the TypeError this whole section exists to remove.
    encodingInfo: function (config) {
      if (config === null || typeof config !== 'object') {
        return mcInvalid('encodingInfo requires a MediaEncodingConfiguration');
      }
      if (config.type !== 'record' && config.type !== 'webrtc') {
        return mcInvalid("encodingInfo: type must be 'record' or 'webrtc'");
      }
      if (!config.audio && !config.video) {
        return mcInvalid('encodingInfo: at least one of audio or video is required');
      }
      return g.Promise.resolve({
        supported: false,
        smooth: false,
        powerEfficient: false,
        configuration: config,
      });
    },
  };

  // ══ EME: the interfaces exist, and NOTHING is ever granted (tick 641) ═══════════════════════
  //
  // **This is not EME.** `CONSTITUTION.MD` PART IV makes *"Widevine/EME HD streaming"* a permanent
  // non-goal — a licensing wall, correctly never chased — and in the same sentence prescribes how
  // to hold it: *"Documented, DEGRADED GRACEFULLY, never chased."* Omitting the interface objects
  // is not graceful degradation. t640 measured what it actually costs:
  //
  //   shaka-player 4.11.2 `isBrowserSupported()` → **false**, on this clause of its own source:
  //     !(window.MediaKeys && window.navigator && window.navigator.requestMediaKeySystemAccess &&
  //       window.MediaKeySystemAccess && window.MediaKeySystemAccess.prototype.getConfiguration)
  //
  // It reads EME's presence as a proxy for *"is this a real browser"* and refuses to run **even for
  // unencrypted content**. So the absence converted "encrypted video will not play" into
  // "shaka-player will not run at all" — a hard failure on exactly the case PART IV says to degrade
  // gracefully into. Every MSE predicate it checks was already green.
  //
  // **The honesty guard, and it is the whole design.** `requestMediaKeySystemAccess` **NEVER
  // RESOLVES.** There is no CDM in this tree, no key system is supported, and a resolved access
  // object would send a site down a decryption path that ends worse than the refusal did — the
  // "advertise before it works" failure MEDIA.md names, wearing DRM's clothes. `NotSupportedError`
  // is the spec's own answer for "no supported configuration", and it is what Chrome without a CDM
  // returns. Netflix and Spotify remain unreachable; they were unreachable before, and the
  // difference is that a clear-content player now boots.
  //
  // The constructors are defined so `instanceof` and prototype feature-detects behave, and their
  // methods reject rather than throw — a rejected promise is a path a player handles; a TypeError
  // from calling a method on `undefined` is the throw-class that takes the page down (t615).
  function emeRefuse(msg) {
    return g.Promise.reject(new g.DOMException(msg, 'NotSupportedError'));
  }

  function MediaKeySession() {
    throw new g.TypeError('Illegal constructor');
  }
  MediaKeySession.prototype.generateRequest = function () {
    return emeRefuse('no key system is supported');
  };
  MediaKeySession.prototype.load = function () { return emeRefuse('no key system is supported'); };
  MediaKeySession.prototype.update = function () { return emeRefuse('no key system is supported'); };
  MediaKeySession.prototype.close = function () { return emeRefuse('no key system is supported'); };
  MediaKeySession.prototype.remove = function () { return emeRefuse('no key system is supported'); };

  function MediaKeys() {
    throw new g.TypeError('Illegal constructor');
  }
  MediaKeys.prototype.createSession = function () {
    // Synchronous in the spec, so this one THROWS rather than rejecting — and it is unreachable
    // anyway, because the only way to obtain a MediaKeys is through an access object that is never
    // handed out.
    throw new g.DOMException('no key system is supported', 'NotSupportedError');
  };
  MediaKeys.prototype.setServerCertificate = function () {
    return emeRefuse('no key system is supported');
  };

  function MediaKeySystemAccess() {
    throw new g.TypeError('Illegal constructor');
  }
  // The property shaka names explicitly. It is never called — no instance is ever created — but the
  // feature-detect reads it off the PROTOTYPE, which is the whole reason this block exists.
  MediaKeySystemAccess.prototype.getConfiguration = function () {
    throw new g.DOMException('no key system is supported', 'NotSupportedError');
  };
  MediaKeySystemAccess.prototype.createMediaKeys = function () {
    return emeRefuse('no key system is supported');
  };

  if (!g.MediaKeys) { g.MediaKeys = MediaKeys; }
  if (!g.MediaKeySession) { g.MediaKeySession = MediaKeySession; }
  if (!g.MediaKeySystemAccess) { g.MediaKeySystemAccess = MediaKeySystemAccess; }

  if (g.navigator && !g.navigator.requestMediaKeySystemAccess) {
    g.navigator.requestMediaKeySystemAccess = function (keySystem, configs) {
      // The spec's argument validation still runs: a player that passes garbage should learn that
      // from us, not have it masked by the blanket refusal below.
      if (typeof keySystem !== 'string' || keySystem === '') {
        return g.Promise.reject(new g.TypeError('keySystem must be a non-empty string'));
      }
      if (!configs || typeof configs.length !== 'number' || configs.length === 0) {
        return g.Promise.reject(new g.TypeError('supportedConfigurations must be a non-empty list'));
      }
      // And then: no key system, ever. Not Widevine (no CDM and no licence), not PlayReady, and not
      // `org.w3.clearkey` either — Clear Key needs a decryptor this tree does not have, and
      // answering yes to it would be the same lie in a smaller font.
      return emeRefuse("no key system is supported: '" + keySystem + "'");
    };
  }

  if (g.navigator && !g.navigator.mediaCapabilities) {
    Object.defineProperty(g.navigator, 'mediaCapabilities', {
      get: function () { return mediaCapabilities; },
      configurable: true,
      enumerable: true,
    });
  }
})();
"#;
