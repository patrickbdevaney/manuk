# The agent picked the first substring match — and the scorer built to do better had no caller

> Landed t1375. Gate: `the_drive_path_picks_the_exact_match_and_the_right_role` (`agent/tests/`).
> Track C.

## The one-sentence mechanism

> **Two halves of one system, built and never joined.** `AgentBrowser::resolve` — behind every
> `click_by_name`, `type_into`, `resolve_handle` and `submit` — called `find_containing`, which is
> *"the first node in tree order whose name CONTAINS the needle"*; and
> `targeting::resolve_target`, the dual semantic+visual scorer with an exact-name bonus and a
> confidence margin, had **no production consumer at all**.

```text
  a page with "Sign in with Google" ABOVE "Sign in"
    find_containing("Sign in")  ->  "Sign in with Google"    the first substring hit
    the dual scorer             ->  "Sign in"                 the exact-name bonus wins
```

⭐ **An agent told to click *Sign in* clicking *Sign in with Google* is not a near miss — it is a
different account**, and on a consent page a different consequence entirely.

This is the t1356 shape one layer up: there, perception (the a11y tree) and actuation
(`dispatch_click_at`) were both built and nothing ran the click point back through the hit-test.
Here, the scorer and the drive path were both built and nothing called one from the other.

## ⚠ And the scorer never saw the ROLE either

`Action::ClickText { role, name }` carries a role, and `action_intent` dropped it — so the scorer
ranked by name and visual salience across every node on the page. `resolve` is always called *with* a
role (`type_into` passes `Role::TextBox`), so scoring without it means *"type into the field called
Search"* can score a **button** called Search.

⚠ The role filter is applied **after** scoring, so the confidence margin is computed against the
candidates that survive it: a runner-up the role excludes is not competition and must not make the
winner look ambiguous.

## ⚠ A low-confidence winner is returned, not refused

Ambiguity has a best answer. That is the difference from t1366's `Obstructed`, where acting is a lie:
two similar buttons still have a most-likely one, and refusing would turn every such page into an
error where the previous behaviour at least picked something. `Grounded::Ambiguous` remains the
surface for a caller that wants to disambiguate before acting.

⚠ `find_containing` is kept as a fallback for an intent that reduces to no keywords (punctuation, an
empty string), so nothing that resolved before stops resolving.

## The gate

⚠ **The decoy is written FIRST in the fixture on purpose**, and a vacuity assert checks that it
really is the first button in tree order — with the exact match first, `find_containing` returns it
too and the gate proves nothing.

- **ARM 1** exact beats an earlier substring (`Sign in` vs `Sign in with Google`).
- **ARM 2** the role scopes the search (`Search` as a `TextBox` finds the field, not the earlier
  button of the same name).
- **ARM 3 CONTROL** a unique name still resolves, to itself — the change is a better *choice* among
  candidates, not a new way to fail.

Proven red by two mutations, each hitting a different arm: restoring `find_containing` fails ARM 1
and leaves ARM 2 green (because `find_containing` *does* filter by role — which is exactly why ARM 2
needs its own mutation); dropping the role filter fails ARM 2 and leaves ARM 1 green.
