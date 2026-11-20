#!/usr/bin/env python3
"""W6.V1 one-shot generator for suite-v1-heldout.json and docs/sprint/w6/V1-set.tsv. After they are written the set is FROZEN
(sha256 in docs/sprint/w6/V1-set.sha256); never re-run to change a task, never tune a task to a result.
Every answer was read off the live page (curl / the page itself) BEFORE any arm ran. Static or historical facts only, so a
live-web drift does not move the answer. No task shares a start URL or fact with the own suite, REAL, the external slice,
BASELINE-URLS or the guard corpora (checked by v1_overlap_check.py)."""
import json
H = lambda i, url, goal, *ans: dict(id=i, slice="heldout", kind="live", start_url=url, goal=goal,
                                    verifier=dict(answer_all=list(ans), answer_none=[]))
W, I, J, X = "https://en.wikipedia.org", "https://the-internet.herokuapp.com", "https://jsonplaceholder.typicode.com", "https://xkcd.com"
T = [
 H("H01", "https://www.rfc-editor.org/rfc/rfc9110", "What is the exact title of this RFC?", "HTTP Semantics"),
 H("H02", "https://www.rfc-editor.org/rfc/rfc2616", "Who is the first author listed for this RFC?", "Fielding"),
 H("H03", "https://www.rfc-editor.org/rfc/rfc791", "What is the title of this RFC?", "Internet Protocol"),
 H("H04", X+"/1/", "What is the title of this xkcd comic?", "Barrel - Part 1"),
 H("H05", X+"/1/", "Click 'Next' to go to the following comic. What is its title?", "Petit Trees (sketch)"),
 H("H06", X+"/353/", "What is the hover (title attribute) text of the comic image? Quote the last sentence.", "Perl, I'm leaving you"),
 H("H07", "https://www.python.org/downloads/release/python-3120/", "What release date does this page give for Python 3.12.0?", "Oct. 2, 2023"),
 H("H08", J+"/users/3", "What is the name of this user?", "Clementine Bauch"),
 H("H09", J+"/users/3", "What is this user's email address?", "Nathan@yesenia.net"),
 H("H10", J+"/todos/5", "What is the title of this todo?", "laboriosam mollitia et enim quasi adipisci quia provident illum"),
 H("H11", "https://www.wikidata.org/wiki/Q42", "What is the date of birth statement for this item?", "11 March 1952"),
 H("H12", "https://www.wikidata.org/wiki/Q42", "What is the place of birth statement for this item?", "Cambridge"),
 H("H13", W+"/wiki/Mount_Everest", "What elevation above sea level does the infobox give, in metres?", "8,848.86"),
 H("H14", W+"/wiki/Linux", "What is the initial release date given in the infobox?", "17 September 1991"),
 H("H15", W+"/wiki/Apollo_11", "On what date was Apollo 11 launched, per the infobox?", "July 16, 1969"),
 H("H16", "https://www.sqlite.org/copyright.html", "Under what licence-like status does this page say SQLite is released?", "public domain"),
 H("H17", "https://pokeapi.co/api/v2/pokemon/ditto", "What is the value of the weight field in this JSON?", "40"),
 H("H18", "https://pokeapi.co/api/v2/pokemon/pikachu", "What is the value of the id field at the top level of this JSON?", "25"),
 H("H19", I+"/hovers", "Hover over the first user avatar. What name is shown?", "user1"),
 H("H20", I+"/dynamic_loading/1", "Click Start and wait for the content to load. What text appears?", "Hello World!"),
 H("H21", "https://www.scrapethissite.com/pages/ajax-javascript/", "Click the year 2015. Which film is marked as the Best Picture winner?", "Spotlight"),
 H("H22", "http://info.cern.ch/hypertext/WWW/TheProject.html", "What is the title (tab title) of this page?", "The World Wide Web project"),
 H("H23", "https://httpbin.org/status/418", "What word appears in brackets in the ASCII art on this page?", "teapot"),
 H("H24", "https://httpbin.org/forms/post", "Fill the customer name with 'Ada', choose size Large, tick Bacon, and submit. In the response, what Content-Type does the echoed request header show?", "application/x-www-form-urlencoded"),
 H("H25", I+"/status_codes", "Click the link for status code 404. What does the page say it returned?", "404 status code"),
 H("H26", I+"/checkboxes", "How many of the checkboxes are checked when the page first loads?", "1"),
 H("H27", I+"/add_remove_elements/", "Click 'Add Element' three times. How many Delete buttons are on the page now?", "3"),
 H("H28", J+"/posts/1", "What is the title of this post?", "sunt aut facere repellat provident occaecati excepturi optio reprehenderit"),
 H("H29", W+"/wiki/Mount_Everest", "In what year was the first confirmed ascent of Mount Everest, per the lead paragraph?", "1953"),
 H("H30", "https://www.rfc-editor.org/rfc/rfc2616", "What month and year was this RFC published?", "June 1999"),
]
assert len(T) == 30 and len({t["id"] for t in T}) == 30
json.dump({"suite": "v1-heldout", "tasks": T}, open("bench/agent-tasks/suite-v1-heldout.json", "w"), indent=1)
with open("docs/sprint/w6/V1-set.tsv", "w") as f:
    f.write("id\tstart_url\tgoal\texpected_all\n")
    for t in T:
        f.write("\t".join([t["id"], t["start_url"], t["goal"], " | ".join(t["verifier"]["answer_all"])]) + "\n")
