#!/usr/bin/env python3
# Gate for RG.2.0.b.a.b.a. Claim: after "+N more" the Month day cells are detached from the DOM (view drilled down
# to the day view), not hidden: no inert/aria-hidden/modal/dialog ancestor anywhere. Usage: gate.py probe.json
import json, sys
d = json.load(open(sys.argv[1])); b, a = d['before'], d['after']
c, k = a['dropped_cell'], a['kept_control']
checks = {
 'before: cell attached, in AX, not ignored, no hidden ancestor': b['dropped_cell']['attached'] and b['dropped_cell']['ax'].get('in_ax') and not b['dropped_cell']['ax'].get('ignored') and b['dropped_cell']['hidden_ancestors'] == 0,
 'after: cell detached from DOM': not c['attached'] and c['month_cells_in_dom'] == 0 and c['ax'].get('in_ax') is False,
 'after: month view unmounted, time view mounted': 'rbc-month-view' not in c['mounted_views'] and 'rbc-time-view' in c['mounted_views'],
 'after: no dialog/overlay/popover': c['dialogs'] == 0,
 'after: kept control attached, in AX, no hidden ancestor (aria-hidden-free sibling is not hidden)': k['attached'] and k['ax'].get('in_ax') and not k['ax'].get('ignored') and k['hidden_ancestors'] == 0,
 'chrome_ver pinned': d['chrome_ver'] == d['pin'] and not d['mutant'],
}
bad = [n for n, ok in checks.items() if not ok]
for n, ok in checks.items(): print('PASS' if ok else 'FAIL', n)
sys.exit(1 if bad else 0)
