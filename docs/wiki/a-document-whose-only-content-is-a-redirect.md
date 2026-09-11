# A document whose only content is a redirect is not the page — and `curl -sL` stops there

**t1504.** `curl -sL` follows `Location:`. A browser follows three more things, and the fidelity
oracle — which `curl`s a document and renders the *snapshot* from `file://` so both engines get the
same bytes — followed only the first. t1486 taught it `<meta http-equiv="refresh">`. This tick
teaches it the other spelling: **a script that navigates and does nothing else.**

## The measurement came first, and it set the rule

The 200-site CrUX trend corpus, every document under 20 KB carrying a top-level navigation
assignment:

```text
  FOLLOW  venus.zeronline.cloud        86 B    0 tags    0 words   -> /administrator/
  FOLLOW  house.udn.com               195 B    0 tags    0 words   -> /house/index
  REFUSE  packages.booking.com       5631 B    7 tags   35 words   -> a PerimeterX captcha closer
  REFUSE  swiftspinus.com            4204 B   33 tags  226 words   -> the literal `https://`
  REFUSE  admin.munchbakery.com     11621 B   67 tags 3666 words   -> a conditional login guard
```

⭐⭐⭐ **The discriminator is not "does it navigate" — it is "does it render anything at all".** A
`location.href =` is in a large fraction of real pages: in a consent handler, a captcha closer, a
login guard. Following any of those sends the instrument to a page the user never sees, which is the
*opposite* of the bug being fixed. Both followed documents carry **zero** words of text outside
`<script>` and **zero** elements that paint, so the budget is zero rather than a threshold chosen to
fit — and a document that grows one visible element stops being followed.

A false NEGATIVE here is the honest refusal the row already had. A false POSITIVE silently scores the
wrong page. The rule is set to make only the first mistake.

## Three refusals the measurement wrote, and one it exposed

`swiftspinus.com` redirects with `location.replace("https://" + e + location.pathname + ...)`, and
the first version of the literal reader returned **`https://`** — a scheme with no host, which
`resolve_url` would have handed to `curl`. ⚠ **A concatenated target is computed, and the literal is
only the target when nothing is glued to it.** The refusal is in the code because a real corpus row
put it there, not because it was foreseen.

`admin.munchbakery.com` puts its navigation in an `onclick` **attribute**, not a `<script>`: refused
twice over. `packages.booking.com` renders a title and a body.

## ⚠ A guard checked on one end of a token is not a guard on the token

The whole-token test originally checked only the character *after* `location`. `geolocation.href =
"/x"` navigated — every byte of the trailing ` = "` is exactly what a real navigation looks like.
Found by the gate's own fixture on the first run.

## ⚠ And one guard was INERT — a green mutation deleted it

A clause refusing `==`, `===` and `=>` by name was written, and deleting it left the gate **green**.
The reason belongs where a future reader will change it: `string_literal_at` requires a QUOTE at the
first non-whitespace byte after the `=`, and the second character of every comparison and arrow is
`=` or `>`. The comparison was already refused one layer down, by the thing that reads the target.
The clause is gone and the coupling is recorded in both the code and the gate. (t1403's rule, third
instance.)

## ⭐⭐⭐ THE RESULT IS NOT A WIN — IT IS A RELABELLING, AND THAT IS THE POINT

Same binary, the two sites, before and after:

```text
                            BEFORE                      AFTER
  house.udn.com             shell-only-1                thin-overlap-1   oracle 1 -> 441 elements
  venus.zeronline.cloud     probe-blocked               render-failed    oracle 0 ->  26 elements
```

Neither site became SCORED. Both moved from a tag that means *"the ORACLE failed, not our bug"* to a
tag whose own text says **"Unlike shell-only this is OURS: the oracle built the page and we did
not."**

That contradicts a conclusion this loop banked two ticks ago. The t1485–1495 refusal histogram read
**ORIGIN 58 / METHOD 23 / ENGINE 6**, and the constitution check's steer was *"METHOD's remainder is
now understood and mostly NOT ours."* Two of those METHOD rows became ENGINE rows the moment the
instrument could see the page behind the stub.

> **A methodology label is a hypothesis about whose bug it is, and an instrument that cannot reach
> the page cannot test it.** `ENGINE 6` was not small because the engine is nearly right on that
> cohort; it was small partly because two of its members were filed under the instrument's own
> blindness.

## Where it is

`tests/wpt/src/chrome.rs` — `script_redirect_target`, `renders_nothing_but_a_script`,
`visible_text_len`, `navigation_assignment`, `string_literal_at`; `fetch_document_following_refresh`
is now `fetch_document_following_redirects` and tries the script form only when there is no
`<meta refresh>`. Gate:
`only_a_document_that_renders_nothing_is_followed_to_its_script_redirect` +
`a_navigation_assignment_is_a_window_location_write_and_nothing_that_resembles_one`, red under five
mutations.

## Banked next, with the evidence already taken

`awlyaa.education.dz` is filed `shell-only-6`. It is an **F5 BIG-IP ASM block page served with HTTP
200**: `<title>Request Rejected</title>` … `Your support ID is: 3937191496790635609`, 247 bytes, 6
tags. `classify_fetch`'s 200-status wall test knows only Cloudflare's two infrastructure markers, so
this vendor falls through to *measurable* and the row is charged to the method. The same survey found
no other unclassified 200-status wall in the 200-site corpus — DataDome's marker appears inside a
981 KB **real page** and must not be used, which is the discipline `CHALLENGE_INFRA` already states.
