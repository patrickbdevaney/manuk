# An ordinal needs an enumeration

t1471 priced the agent's addressing gap and found the whole of it is naming, not geometry:

```text
                   rate    +landmark   +heading   ceiling(ordinal)
  TOTAL           78.5%       81.7%      84.3%        99.5%
```

At a **99.5% ceiling** essentially every target is already grounded and unoccluded, so the shortfall
is entirely *which one did you mean* — and an ordinal recovers **15.2** of the 21 points against the
two semantic terms' 5.8.

⭐⭐⭐ **But an agent can only ask for *the third `Edit`* if something told it there are three.** With
`nth` and no enumeration, an ambiguous resolve is still a dead end: the caller is handed one
arbitrary winner and cannot see the set it came from. The measured 15.2 points were unreachable by
any caller.

`targeting::candidates` / `AgentBrowser::candidates_for` publish the set, each row carrying the terms
that tell them apart:

```text
  nth  role  name    landmark      heading    point
   0   link  Edit    navigation    –          (80, 12)
   1   link  Edit    –             History    (80, 92)
   2   link  Edit    –             Usage      (80, 172)
```

## The round trip is the whole point

**The published order must be the order `nth` indexes.** Both sort by node id — document order — so
`candidates(..)[i].node == resolve_target_at(.., nth = Some(i), ..)` by construction, and the gate
asserts it for every row.

Publishing one order and indexing another would be the t1402 shape: two halves of one system that
disagree about the thing they share, each with tests that pass. An enumeration the ordinal does not
match is worse than none, because the caller has no way to notice.

## An enumeration of identical rows is not an enumeration

Every row publishes its `landmark`, its nearest preceding `heading` and its click point. Three rows
reading `link "Edit"` and nothing else would be the same dead end with a length attached — so the
gate asserts that the rows *differ*, and that the terms they publish actually work as addresses when
handed back to `resolve_target_at`.

## ⚠ The role filter was untested until a button was added

Mutation 2 — dropping the role filter from `candidates` — came back **green**. With only links named
`Edit` on the page, nothing else scored above zero, so the filter changed no answer. A same-named
element of a **different role** (`<button>Edit</button>`) is the only thing that can see it. That
button is in the fixture because the mutation was run.

See also [[the-whole-gap-is-addressing]], [[the-landmark-is-the-missing-term]].
