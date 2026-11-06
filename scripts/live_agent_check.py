"""W3.17.a gate checker. usage: live_agent_check.py run|esc <shot.xwd> <logfile>
Fails (exit 1) when the mid-run screenshot's page area is a blank white window, when real-loop
redraw p95 >= 50 ms or a redraw gap > 500 ms, or (esc) when more than 1 step ran after Esc."""
import re, struct, sys
from PIL import Image

mode, shot, log = sys.argv[1:4]
raw = open(shot, "rb").read()
hdr = struct.unpack(">25I", raw[:100])
hsize, w, h, bpp, bpl, ncol = hdr[0], hdr[4], hdr[5], hdr[11], hdr[12], hdr[19]
off = hsize + ncol * 12
img = Image.frombuffer("RGBA", (w, h), raw[off:off + bpl * h], "raw", "BGRA", bpl, 1).convert("RGB")
img.save(shot.replace(".xwd", ""))
page = img.crop((0, 150, w, h - 20))
px = list(page.getdata())
white = sum(1 for r, g, b in px if r > 245 and g > 245 and b > 245) / len(px)
m = re.search(r"frames=(\d+) p95_ms=([\d.]+) max_gap_ms=([\d.]+) steps=(\d+) steps_after_esc=(-?\d+)", open(log).read())
assert m, "no [live-frames] line: the run never ended"
frames, p95, gap, steps, after = int(m[1]), float(m[2]), float(m[3]), int(m[4]), int(m[5])
print(f"mid-run page area white fraction={white:.3f} frames={frames} p95_ms={p95} max_gap_ms={gap} steps={steps} steps_after_esc={after}")
ok = white < 0.8 and p95 < 50 and gap < 500
if mode == "esc":
    ok = ok and 0 <= after <= 1 and steps < 11
sys.exit(0 if ok else 1)
