## RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b.b.a.a

chrome_ver=145.0.7632.116. 1 replay per backend of the first 15 opendining-4 steps; ordinal steps: [1, 13, 14].

### step #1: transcript `[h6] textbox =Jul 18 [readonly]` = textbox ordinal 0

- manuk: resolved; 2 textbox nodes in the observation after the step; around ordinal 0:
    [h4] textbox =Jul 18 [readonly]
    [h9] textbox "Location, Restaurant, or Cuisine"
- chrome: resolved; 2 textbox nodes in the observation after the step; around ordinal 0:
    [h3] textbox =Jul 18 [readonly]
    [h8] textbox "Location, Restaurant, or Cuisine" =

### step #13: transcript `[h8] combobox [collapsed]` = combobox ordinal 1

- manuk: UNRESOLVED; 0 combobox nodes in the observation it failed against; around ordinal 1:
- chrome: resolved; 4 combobox nodes in the observation after the step; around ordinal 1:
    [h4] combobox "Select time"
    [h5] combobox [expanded]
    [h6] combobox "Select number of guests"
    [h7] combobox [collapsed]

### step #14: transcript `[h10] combobox [collapsed]` = combobox ordinal 3

- manuk: UNRESOLVED; 0 combobox nodes in the observation it failed against; around ordinal 3:
- chrome: resolved; 4 combobox nodes in the observation after the step; around ordinal 3:
    [h5] combobox [expanded]
    [h6] combobox "Select number of guests"
    [h7] combobox [collapsed]

FIRST DIVERGING STEP: #13 (`act ('#ord', 'combobox', 1)`); transcript ordinal 1 of combobox; manuk has 0 nodes of that role, chrome 4.

manuk interactive observation at that step:
```
[h336] button [clickable] ↓offscreen
[h337] option "12:00 AM"
[h338] option "12:30 AM"
[h339] option "1:00 AM"
[h340] option "1:30 AM"
[h341] option "2:00 AM"
[h342] option "2:30 AM"
[h343] option "3:00 AM"
[h344] option "3:30 AM"
[h345] option "4:00 AM"
[h346] option "4:30 AM"
[h347] option "5:00 AM"
[h348] option "5:30 AM"
[h349] option "6:00 AM"
[h350] option "6:30 AM"
[h351] option "7:00 AM"
[h352] option "7:30 AM"
[h353] option "8:00 AM"
[h354] option "8:30 AM"
[h355] option "9:00 AM"
[h356] option "9:30 AM"
[h357] option "10:00 AM" ↓offscreen
[h358] option "10:30 AM" ↓offscreen
[h359] option "11:00 AM" ↓offscreen
[h360] option "11:30 AM" ↓offscreen
[h361] option "12:00 PM" ↓offscreen
[h362] option "12:30 PM" ↓offscreen
[h363] option "1:00 PM" [selected] ↓offscreen
[h364] option "1:30 PM" ↓offscreen
[h365] option "2:00 PM" ↓offscreen
[h366] option "2:30 PM" ↓offscreen
[h367] option "3:00 PM" ↓offscreen
[h368] option "3:30 PM" ↓offscreen
[h369] option "4:00 PM" ↓offscreen
[h370] option "4:30 PM" ↓offscreen
[h371] option "5:00 PM" ↓offscreen
[h372] option "5:30 PM" ↓offscreen
[h373] option "6:00 PM" ↓offscreen
[h374] option "6:30 PM" ↓offscreen
[h375] option "7:00 PM" ↓offscreen
[h376] option "7:30 PM" ↓offscreen
[h377] option "8:00 PM" ↓offscreen
[h378] option "8:30 PM" ↓offscreen
[h379] option "9:00 PM" ↓offscreen
[h380] option "9:30 PM" ↓offscreen
[h381] option "10:00 PM" ↓offscreen
[h382] option "10:30 PM" ↓offscreen
[h383] option "11:00 PM" ↓offscreen
[h384] option "11:30 PM" ↓offscreen
[h385] button [clickable] ↓offscreen
```

chrome interactive observation at that step:
```
[h2] button "Home"
[h3] textbox =Nov 2 [readonly]
[h4] combobox "Select time"
[h5] combobox [expanded]
[h6] combobox "Select number of guests"
[h7] combobox [collapsed]
[h8] textbox "Location, Restaurant, or Cuisine" =
[h9] button "Let's go"
[h10] link "View details for The Royal Dine" in listitem "The Royal Dine 20 reviews French . $$$$ …"
[h11] button "The Royal Dine 20 reviews French . $$$$ . Mission District Booked 24 times today…"
[h12] link "Book for 1:30 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …"
[h13] button "1:30 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …"
[h14] link "Book for 2:00 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …"
[h15] button "2:00 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …"
[h16] link "Book for 2:30 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …"
[h17] button "2:30 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …"
[h18] link "View details for Vintage Bites" in listitem "Vintage Bites 20 reviews American . $$$$…"
[h19] button "Vintage Bites 20 reviews American . $$$$ . SOMA Booked 24 times today 1:30 PM 2:…"
[h20] link "Book for 1:30 PM" in listitem "Vintage Bites 20 reviews American . $$$$…"
[h21] button "1:30 PM" in listitem "Vintage Bites 20 reviews American . $$$$…"
[h22] link "Book for 2:00 PM" in listitem "Vintage Bites 20 reviews American . $$$$…"
[h23] button "2:00 PM" in listitem "Vintage Bites 20 reviews American . $$$$…"
[h24] link "Book for 2:30 PM" in listitem "Vintage Bites 20 reviews American . $$$$…"
[h25] button "2:30 PM" in listitem "Vintage Bites 20 reviews American . $$$$…"
[h26] link "View details for Evening Delight" in listitem "Evening Delight 20 reviews Italian . $$$…"
[h27] button "Evening Delight 20 reviews Italian . $$$$ . North Beach Booked 24 times today 1:…"
[h28] link "Book for 1:30 PM" in listitem "Evening Delight 20 reviews Italian . $$$…"
[h29] button "1:30 PM" in listitem "Evening Delight 20 reviews Italian . $$$…"
[h30] link "Book for 2:00 PM" in listitem "Evening Delight 20 reviews Italian . $$$…"
[h31] button "2:00 PM" in listitem "Evening Delight 20 reviews Italian . $$$…"
[h32] link "Book for 2:30 PM" in listitem "Evening Delight 20 reviews Italian . $$$…"
[h33] button "2:30 PM" in listitem "Evening Delight 20 reviews Italian . $$$…"
[h34] link "View details for River View Café" in listitem "River View Café 20 reviews Café . $$$$ .…"
[h35] button "River View Café 20 reviews Café . $$$$ . Embarcadero Booked 24 times today 1:30 …"
[h36] link "Book for 1:30 PM" in listitem "River View Café 20 reviews Café . $$$$ .…"
[h37] button "1:30 PM" in listitem "River View Café 20 reviews Café . $$$$ .…"
[h38] link "Book for 2:00 PM" in listitem "River View Café 20 reviews Café . $$$$ .…"
[h39] button "2:00 PM" in listitem "River View Café 20 reviews Café . $$$$ .…"
[h40] button "Next slide"
[h41] button "Explore Icon restaurants"
[h42] link "View details for The Royal Dine" in listitem "The Royal Dine 20 reviews French . $$$$ …"
[h43] button "The Royal Dine 20 reviews French . $$$$ . Mission District Booked 24 times today…"
[h44] link "Book for 1:30 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …" ↓offscreen
[h45] button "1:30 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …" ↓offscreen
[h46] link "Book for 2:00 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …" ↓offscreen
[h47] button "2:00 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …" ↓offscreen
[h48] link "Book for 2:30 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …" ↓offscreen
[h49] button "2:30 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …" ↓offscreen
[h50] link "View details for Vintage Bites" in listitem "Vintage Bites 20 reviews American . $$$$…"
[h51] button "Vintage Bites 20 reviews American . $$$$ . SOMA Booked 24 times today 1:30 PM 2:…"
[h52] link "Book for 1:30 PM" in listitem "Vintage Bites 20 reviews American . $$$$…" ↓offscreen
[h53] button "1:30 PM" in listitem "Vintage Bites 20 reviews American . $$$$…" ↓offscreen
[h54] link "Book for 2:00 PM" in listitem "Vintage Bites 20 reviews American . $$$$…" ↓offscreen
[h55] button "2:00 PM" in listitem "Vintage Bites 20 reviews American . $$$$…" ↓offscreen
[h56] link "Book for 2:30 PM" in listitem "Vintage Bites 20 reviews American . $$$$…" ↓offscreen
[h57] button "2:30 PM" in listitem "Vintage Bites 20 reviews American . $$$$…" ↓offscreen
[h58] link "View details for Evening Delight" in listitem "Evening Delight 20 reviews Italian . $$$…"
[h59] button "Evening Delight 20 reviews Italian . $$$$ . North Beach Booked 24 times today 1:…"
[h60] link "Book for 1:30 PM" in listitem "Evening Delight 20 reviews Italian . $$$…" ↓offscreen
[h61] button "1:30 PM" in listitem "Evening Delight 20 reviews Italian . $$$…" ↓offscreen
[h62] link "Book for 2:00 PM" in listitem "Evening Delight 20 reviews Italian . $$$…" ↓offscreen
[h63] button "2:00 PM" in listitem "Evening Delight 20 reviews Italian . $$$…" ↓offscreen
[h64] link "Book for 2:30 PM" in listitem "Evening Delight 20 reviews Italian . $$$…" ↓offscreen
[h65] button "2:30 PM" in listitem "Evening Delight 20 reviews Italian . $$$…" ↓offscreen
[h66] link "View details for River View Café" in listitem "River View Café 20 reviews Café . $$$$ .…"
[h67] button "River View Café 20 reviews Café . $$$$ . Embarcadero Booked 24 times today 1:30 …"
[h68] link "Book for 1:30 PM" in listitem "River View Café 20 reviews Café . $$$$ .…" ↓offscreen
[h69] button "1:30 PM" in listitem "River View Café 20 reviews Café . $$$$ .…" ↓offscreen
[h70] link "Book for 2:00 PM" in listitem "River View Café 20 reviews Café . $$$$ .…" ↓offscreen
[h71] button "2:00 PM" in listitem "River View Café 20 reviews Café . $$$$ .…" ↓offscreen
[h72] button "Next slide" ↓offscreen
[h73] link "View details for The Royal Dine" in listitem "The Royal Dine 20 reviews French . $$$$ …" ↓offscreen
[h74] button "The Royal Dine 20 reviews French . $$$$ . Mission District Booked 24 times today…" ↓offscreen
[h75] link "Book for 1:30 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …" ↓offscreen
[h76] button "1:30 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …" ↓offscreen
[h77] link "Book for 2:00 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …" ↓offscreen
[h78] button "2:00 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …" ↓offscreen
[h79] link "Book for 2:30 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …" ↓offscreen
[h80] button "2:30 PM" in listitem "The Royal Dine 20 reviews French . $$$$ …" ↓offscreen
[h81] link "View details for Vintage Bites" in listitem "Vintage Bites 20 reviews American . $$$$…" ↓offscreen
[h82] button "Vintage Bites 20 reviews American . $$$$ . SOMA Booked 24 times today 1:30 PM 2:…" ↓offscreen
[h83] link "Book for 1:30 PM" in listitem "Vintage Bites 20 reviews American . $$$$…" ↓offscreen
[h84] button "1:30 PM" in listitem "Vintage Bites 20 reviews American . $$$$…" ↓offscreen
[h85] link "Book for 2:00 PM" in listitem "Vintage Bites 20 reviews American . $$$$…" ↓offscreen
[h86] button "2:00 PM" in listitem "Vintage Bites 20 reviews American . $$$$…" ↓offscreen
[h87] link "Book for 2:30 PM" in listitem "Vintage Bites 20 reviews American . $$$$…" ↓offscreen
[h88] button "2:30 PM" in listitem "Vintage Bites 20 reviews American . $$$$…" ↓offscreen
[h89] link "View details for Evening Delight" in listitem "Evening Delight 20 reviews Italian . $$$…" ↓offscreen
[h90] button "Evening Delight 20 reviews Italian . $$$$ . North Beach Booked 24 times today 1:…" ↓offscreen
[h91] link "Book for 1:30 PM" in listitem "Evening Delight 20 reviews Italian . $$$…" ↓offscreen
[h92] button "1:30 PM" in listitem "Evening Delight 20 reviews Italian . $$$…" ↓offscreen
[h93] link "Book for 2:00 PM" in listitem "Evening Delight 20 reviews Italian . $$$…" ↓offscreen
[h94] button "2:00 PM" in listitem "Evening Delight 20 reviews Italian . $$$…" ↓offscreen
[h95] link "Book for 2:30 PM" in listitem "Evening Delight 20 reviews Italian . $$$…" ↓offscreen
[h96] button "2:30 PM" in listitem "Evening Delight 20 reviews Italian . $$$…" ↓offscreen
[h97] link "View details for River View Café" in listitem "River View Café 20 reviews Café . $$$$ .…" ↓offscreen
[h98] button "River View Café 20 reviews Café . $$$$ . Embarcadero Booked 24 times today 1:30 …" ↓offscreen
[h99] link "Book for 1:30 PM" in listitem "River View Café 20 reviews Café . $$$$ .…" ↓offscreen
[h100] button "1:30 PM" in listitem "River View Café 20 reviews Café . $$$$ .…" ↓offscreen
[h101] link "Book for 2:00 PM" in listitem "River View Café 20 reviews Café . $$$$ .…" ↓offscreen
[h102] button "2:00 PM" in listitem "River View Café 20 reviews Café . $$$$ .…" ↓offscreen
[h103] button "Next slide" ↓offscreen
[h104] link "Link to Fancy Lights Bistro's details page" ↓offscreen
[h105] link "View details of Evening Delight" ↓offscreen
[h106] link "View details of Vintage Bites" ↓offscreen
[h156] button [clickable]
[h157] option "12:00 AM" ↑offscreen
[h158] option "12:30 AM" ↑offscreen
[h159] option "1:00 AM" ↑offscreen
[h160] option "1:30 AM" ↑offscreen
[h161] option "2:00 AM" ↑offscreen
[h162] option "2:30 AM" ↑offscreen
[h163] option "3:00 AM" ↑offscreen
[h164] option "3:30 AM" ↑offscreen
[h165] option "4:00 AM" ↑offscreen
[h166] option "4:30 AM" ↑offscreen
[h167] option "5:00 AM" ↑offscreen
[h168] option "5:30 AM" ↑offscreen
[h169] option "6:00 AM" ↑offscreen
[h170] option "6:30 AM" ↑offscreen
[h171] option "7:00 AM"
[h172] option "7:30 AM"
[h173] option "8:00 AM"
[h174] option "8:30 AM"
[h175] option "9:00 AM"
[h176] option "9:30 AM"
[h177] option "10:00 AM"
[h178] option "10:30 AM"
[h179] option "11:00 AM"
[h180] option "11:30 AM"
[h181] option "12:00 PM"
[h182] option "12:30 PM"
[h183] option "1:00 PM" [selected]
[h184] option "1:30 PM"
[h185] option "2:00 PM"
[h186] option "2:30 PM"
[h187] option "3:00 PM"
[h188] option "3:30 PM"
[h189] option "4:00 PM"
[h190] option "4:30 PM"
[h191] option "5:00 PM"
[h192] option "5:30 PM"
[h193] option "6:00 PM"
[h194] option "6:30 PM" ↓offscreen
[h195] option "7:00 PM" ↓offscreen
[h196] option "7:30 PM" ↓offscreen
[h197] option "8:00 PM" ↓offscreen
[h198] option "8:30 PM" ↓offscreen
[h199] option "9:00 PM" ↓offscreen
[h200] option "9:30 PM" ↓offscreen
[h201] option "10:00 PM" ↓offscreen
[h202] option "10:30 PM" ↓offscreen
[h203] option "11:00 PM" ↓offscreen
[h204] option "11:30 PM" ↓offscreen
```
