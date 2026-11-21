## RG.3.0.b.a.a.a.a.a.a.a.b

chrome_ver=145.0.7632.116. opendining-7: replayed step list (flatten of the transcript) beside the transcript calls.

Transcript calls: call 1 navigate; call 2 act; call 3 fill_form; call 4 observe; call 5 type; call 6 observe; call 7 type; call 8 observe; call 9 act; call 10 observe

Replayed steps: #0 navigate ; #1 act h47; #2 type h150; #3 type h150; #4 act h155

Replay tool list in rg3b_replay.flatten: act type key navigate scroll select check press commit back (no fill_form).

- manuk #2: transcript `[h150] textbox =+55 501 001 23 [invalid]` live `[h148] textbox =+1` match=no
- manuk #3: transcript `[h150] textbox =+41 55 550 12 3` live `[h148] textbox =+41 55 550 12 3` match=yes
- chrome #2: transcript `[h150] textbox =+55 501 001 23 [invalid]` live `[h4] textbox =+1` match=no
- chrome #3: transcript `[h150] textbox =+41 55 550 12 3` live `[h4] textbox =+41 55 550 12 3` match=yes
