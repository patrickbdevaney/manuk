"""R1.4.a.d.b.a.a: an fd-level line reader with a deadline for the stdio MCP clients in bench/ and scripts/.

readline() blocks forever on a server that never answers; a hang then loses a measured run without an ERROR row.
This reads the fd itself (no hidden buffer select() cannot see) and raises TimeoutError at the deadline.
Override the deadline with MANUK_RPC_DEADLINE (seconds, default 50).
"""
import os, select, time

DEADLINE = float(os.environ.get("MANUK_RPC_DEADLINE", "50"))


def readline(p, deadline=None):
    """Next stdout line of Popen `p` as str; '' at EOF; TimeoutError if no full line arrives within the deadline."""
    end = time.time() + (DEADLINE if deadline is None else deadline)
    buf = getattr(p, "_mcp_buf", b"")
    fd = p.stdout.fileno()
    while b"\n" not in buf:
        left = end - time.time()
        if left <= 0 or not select.select([fd], [], [], left)[0]:
            p._mcp_buf = buf
            raise TimeoutError(f"no answer from MCP server within {DEADLINE if deadline is None else deadline}s")
        chunk = os.read(fd, 65536)
        if not chunk:
            p._mcp_buf = b""
            return buf.decode("utf-8") if buf else ""
        buf += chunk
    raw, _, p._mcp_buf = buf.partition(b"\n")
    return raw.decode("utf-8") + "\n"
