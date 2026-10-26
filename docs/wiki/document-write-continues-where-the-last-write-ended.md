# document.write continues where the last write ended

Each `document.write` from one script inserts after the previous write's last node (`WRITE_CURSOR` in
`engine/js/src/dom_bindings.rs`), not `afterend` of the script every time — that reversed a loop's
output (W3 W2 W1). Markup that is body-level (anything but script/style/link/meta/title/base/noscript/
template, or non-blank text) written from a `<head>` script goes to the start of `<body>`, as the
parser would have closed `<head>`; inside `<head>` it never renders. Gate: `engine/page/tests/g_document_write_head.rs`
(red before: head case dumped just `BODY`). Divergence kept: a write after load appends, never wipes.
