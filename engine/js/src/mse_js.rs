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
    // (`mp4a.40.*`) demuxes+decodes to PCM (G_MEDIA_AAC). WebM/VP9/AV1 stay false — no demuxer,
    // no decoder, and a YES here without one steers a player onto a path that hangs (module doc).
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
      return false;
    }
    return true;
  };

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
    if (ms && info.duration > 0 && !(ms.__duration > 0)) { ms.__duration = info.duration; }
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
        self.__duration = v;
      }
    });
  }
  target(MediaSource.prototype);

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
})();
"#;
