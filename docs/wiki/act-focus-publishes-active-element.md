# `act focus` must publish the focused node into the JS world

> CB.1.a.a. Gate: `mcp/tests/backends.rs` `every_tool_on_the_manuk_backend` (focus probe).

**Mechanism.** `Page::set_focus` updated `Dom::focused` (so `:focus` matched) but `document.activeElement`
reads a separate JS-side cell (`ACTIVE_ELEMENT`), written only by `publish_view_state`, which GUI hosts call
every frame and the agent never does. A page timer polling `document.activeElement` therefore never saw an
`act focus`. `set_focus` now calls `manuk_js::set_active_element(node)` before dispatching blur/focusout/
focusin/focus, so handlers and later timers read the new focus.

**Lesson.** State mirrored into the JS world needs a single writer inside the engine operation, not a
per-host publish call.
