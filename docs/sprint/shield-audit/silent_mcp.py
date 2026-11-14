#!/usr/bin/env python3
"""A stand-in MCP server that reads stdin and never answers (R1.4.a.d.b.a.a)."""
import sys
for _ in sys.stdin:
    pass
