//! **G_MEDIA_SESSION — `navigator.mediaSession` + `MediaMetadata` retain state and action handlers.**
//!
//! The Media Session API is what every media site drives: YouTube, Spotify, SoundCloud, Netflix,
//! podcast players and every `<audio>`-backed app set
//! `navigator.mediaSession.metadata = new MediaMetadata({title, artist, artwork})` and wire
//! `navigator.mediaSession.setActionHandler('play'|'pause'|'nexttrack'|…, fn)` for OS media keys, the
//! lock screen and headset buttons. Much of this code does NOT guard `navigator.mediaSession` — it is
//! assumed present, like `geolocation` — so its absence is a silent-handler failure:
//! `navigator.mediaSession.setActionHandler is not a function` throws out of the player's init and the
//! player is dead.
//!
//! We have no OS media-key surface to invoke the handlers from (a host integration; stated as the
//! honest limit), but the API is REAL, not inert — it retains everything, so the site's setup runs and
//! the state is queryable/actuable. The gate drives that:
//!
//!   1. `navigator.mediaSession` + `MediaMetadata` exist; `setActionHandler`/`setPositionState` are
//!      callable and the whole setup does not throw.
//!   2. `metadata = new MediaMetadata({...})` ROUND-TRIPS — title/artist read back, and `artwork` is
//!      normalized to an array whose `[0].src` is the provided URL (the shape sites read back).
//!   3. `playbackState` round-trips ('playing').
//!   4. An out-of-enum action REJECTS with a TypeError (a silently-accepted typo would hide the bug).
//!   5. A registered handler is RETAINED and invocable — the `__invoke` seam (what a host/agent uses)
//!      runs the stored `play` handler, proving this is not a `typeof`-passing inert stub.
//!
//! RED: removing the mediaSession shim drops `defined`, `metadata`, `handlerran` together — the setup
//! throws on `undefined.setActionHandler`, the exact dead-player failure a missing API produces.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } }
    };
    try {
      var ms = navigator.mediaSession;
      R.push('defined:' + (ms && typeof ms.setActionHandler === 'function' &&
                           typeof ms.setPositionState === 'function' &&
                           typeof MediaMetadata === 'function'));

      ms.metadata = new MediaMetadata({
        title: 'Song', artist: 'Band', album: 'LP',
        artwork: [{ src: 'https://cdn.test/a.png', sizes: '96x96', type: 'image/png' }]
      });
      R.push('metadata:' + (ms.metadata.title === 'Song' && ms.metadata.artist === 'Band' &&
                            ms.metadata.artwork.length === 1 &&
                            ms.metadata.artwork[0].src === 'https://cdn.test/a.png'));

      ms.playbackState = 'playing';
      R.push('playbackstate:' + (ms.playbackState === 'playing'));

      ms.setPositionState({ duration: 200, position: 40, playbackRate: 1 });

      // Out-of-enum action must throw.
      var threw = false;
      try { ms.setActionHandler('not-a-real-action', function () {}); }
      catch (e) { threw = (e instanceof TypeError); }
      R.push('enumrejected:' + threw);

      // A real handler is retained and invocable through the host/agent seam.
      var played = 0;
      ms.setActionHandler('play', function () { played++; });
      ms.setActionHandler('pause', function () {});
      var ran = ms.__invoke('play');
      R.push('handlerran:' + (ran === true && played === 1));

      // Unsetting removes it.
      ms.setActionHandler('play', null);
      R.push('handlerunset:' + (ms.__invoke('play') === false));

      R.push('ready:true');
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

#[test]
fn media_session_retains_metadata_and_handlers() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ms.test/", &fonts, 800.0);
    let root = page.dom().root();

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("defined:true", "`navigator.mediaSession` + `MediaMetadata` must exist with setActionHandler/setPositionState callable — players do not guard the object, so its absence throws `undefined.setActionHandler` out of init"),
        ("metadata:true", "`metadata = new MediaMetadata({...})` must round-trip title/artist and normalize artwork to an array whose [0].src is the provided URL — the shape sites read back to render 'now playing'"),
        ("playbackstate:true", "`playbackState` must round-trip so the site (and a host) agree on whether media is playing"),
        ("enumrejected:true", "an out-of-enum action must throw a TypeError — silently accepting a typo would hide the caller's bug"),
        ("handlerran:true", "a registered action handler must be RETAINED and invocable (the host/agent seam runs the stored `play` handler) — proving this is not a typeof-passing inert stub"),
        ("handlerunset:true", "passing null must remove the handler"),
        ("ready:true", "the whole player-setup sequence must complete without throwing"),
    ] {
        assert!(
            got.contains(claim),
            "G_MEDIA_SESSION: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
