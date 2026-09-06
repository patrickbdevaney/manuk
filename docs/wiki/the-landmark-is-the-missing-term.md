# The landmark is the missing term

`drive-probe` measured the agent's addressing problem across six real sites: of the targets it can
perceive but cannot act on, **99% are ambiguous** (516 of 521, against 2 ungrounded and 3 occluded).
Listing them named the mechanism in one run:

```text
  Ambiguous  link  "Posts"        Ambiguous  link  "GitHub"
  Ambiguous  link  "Spotlight"    Ambiguous  link  "Sitemap"
  Ambiguous  link  "About"        Ambiguous  link  "Back to top"
```

Every one appears **twice: once in the header nav, once in the footer.** Chrome's tree has the same
twins, so this is not a projection defect. It is an addressing scheme that cannot express what a
human says without thinking about it: *the `Posts` link **in the navigation***.

## Priced before it was built

The `+landmark` column of `drive-probe` re-keys the address as `(landmark, role, name)` and reports
what that would buy, without any resolver change:

```text
                            rate   +landmark
  martinfowler.com         84.3%       89.3%
  news.ycombinator.com     71.2%       71.2%     ← no landmarks; does not move at all
  www.a11yproject.com      79.3%       79.3%
  en.wikipedia.org/…       61.9%       65.3%
  TOTAL                    80.7%       82.4%
```

`targeting::resolve_target_in` is the mechanism that column was measuring. It filters candidates to
those whose nearest enclosing landmark matches — **after** scoring, for the same reason the role
filter is: a runner-up in another landmark is not competition and must not make the winner look
ambiguous.

## Why the headline rate does not move when you add it

`drive-probe`'s `rate` asks *is `(role, name)` unique?* — a property of the **page**, not of the
resolver. Adding a way to disambiguate does not make the duplicates go away, so the column that
moves is `+landmark`, and it already did. **A capability and the metric that priced it can ask
different questions**; reporting the first as if it moved the second would be the mistake.

## The limits, asserted rather than implied

- **A site with no landmarks does not move at all.** news.ycombinator is `71.2% → 71.2%`.
- **Duplicates inside one landmark are untouched.** Two identical links in `<main>` still resolve at
  low confidence, and the gate asserts that so the landmark cannot be mistaken for a solution.
- **The unscoped call is unchanged.** `landmark: None` skips the filter entirely. A new address that
  silently narrowed the old one would be a regression wearing a feature's clothes.

## A test that asserts the right thing about a link it cannot follow

The fixture's links are `file://` paths to nothing, so activation always errors. Asserting
`is_ok()` would have been satisfied by clicking *either* twin. The nav's `Posts` points at `/posts`
and the footer's at `/posts-archive`, so **the attempted URL in the error names the link that was
actually clicked** — a stronger proof than success would have been.

See also [[role-plus-name-is-not-an-address]], [[the-agents-browser-had-no-javascript]].
