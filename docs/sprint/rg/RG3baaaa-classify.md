## RG.3.0.b.a.a.a.a

chrome_ver=145.0.7632.116. Each unresolved step on the failing backend (replay at settle=2, wait 2 s) is classified against the observation it failed to resolve in: **late** = role+name is in a later observation on the same backend; **renamed** = not late, and the observation has the same role with a prefix-matching name or the transcript name under another role/name; **absent** = neither. **no-name** = the handle never appeared in the transcript.

| task | backend | steps | unresolved | absent | renamed | late | no-name | first gap |
|---|---|---|---|---|---|---|---|---|
| zilloft-2 | manuk | 18 | 2 | 0 | 0 | 2 | 0 | #15 |
| zilloft-2 | chrome | 18 | 7 | 3 | 4 | 0 | 0 | #2 |
| opendining-4 | manuk | 26 | 20 | 4 | 1 | 10 | 5 | #1 |
| opendining-4 | chrome | 26 | 11 | 4 | 0 | 2 | 5 | #1 |
| gomail-3 | manuk | 4 | 0 | 0 | 0 | 0 | 0 | - |
| gomail-3 | chrome | 4 | 1 | 0 | 1 | 0 | 0 | #3 |
| opendining-7 | manuk | 5 | 2 | 0 | 0 | 0 | 2 | #2 |
| opendining-7 | chrome | 5 | 2 | 0 | 0 | 0 | 2 | #2 |
| udriver-9 | manuk | 22 | 0 | 0 | 0 | 0 | 0 | - |
| udriver-9 | chrome | 22 | 7 | 4 | 3 | 0 | 0 | #12 |
| udriver-7 | manuk | 7 | 2 | 2 | 0 | 0 | 0 | #3 |
| udriver-7 | chrome | 7 | 0 | 0 | 0 | 0 | 0 | - |
| gocalendar-2 | manuk | 16 | 10 | 5 | 0 | 0 | 5 | #1 |
| gocalendar-2 | chrome | 16 | 10 | 5 | 0 | 0 | 5 | #1 |
| all | | | 74 | 27 | 9 | 14 | 24 | |

