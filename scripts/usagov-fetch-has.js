// R0.4.a.b.a.a.a.a.a: read a serialized DOM (Chrome --dump-dom with JavaScript disabled) and print, for each
// browser path key given on the command line, whether the element exists in that DOM (RAW) or not (JS_ONLY).
// Path keys use the oracle's format: tag:nth-of-type(i) segments joined by '/', rooted at <html>'s children.
// usage: node usagov-fetch-has.js DOM_FILE KEYS_FILE   (KEYS_FILE: one key per line)
'use strict';
const fs = require('fs');
const [, , domFile, keysFile] = process.argv;
const src = fs.readFileSync(domFile, 'utf8');
const VOID = new Set('area base br col embed hr img input link meta param source track wbr'.split(' '));
const RAW = new Set(['script', 'style', 'textarea', 'title']);
const root = { tag: '#root', children: [] };
const stack = [root];
const tok = /<!--[\s\S]*?-->|<!doctype[^>]*>|<\/([a-zA-Z][\w:-]*)\s*>|<([a-zA-Z][\w:-]*)((?:[^>"']|"[^"]*"|'[^']*')*)>/gi;
let m;
while ((m = tok.exec(src))) {
  if (m[1]) {                       // end tag: pop to the nearest open element of that name, ignore strays
    const name = m[1].toLowerCase();
    for (let i = stack.length - 1; i > 0; i--) if (stack[i].tag === name) { stack.length = i; break; }
    continue;
  }
  if (!m[2]) continue;              // comment or doctype
  const tag = m[2].toLowerCase();
  const el = { tag, children: [] };
  stack[stack.length - 1].children.push(el);
  if (VOID.has(tag) || /\/\s*$/.test(m[3])) continue;
  if (RAW.has(tag)) {               // raw text: skip to the matching end tag, do not parse inside
    const close = new RegExp('</' + tag + '\\s*>', 'gi');
    close.lastIndex = tok.lastIndex;
    const c = close.exec(src);
    tok.lastIndex = c ? close.lastIndex : src.length;
    continue;
  }
  stack.push(el);
}
const have = new Set();
function walk(el, prefix) {
  const seen = {};
  for (const c of el.children) {
    seen[c.tag] = (seen[c.tag] || 0) + 1;
    const seg = `${c.tag}:nth-of-type(${seen[c.tag]})`;
    const p = prefix ? `${prefix}/${seg}` : seg;
    have.add(p);
    walk(c, p);
  }
}
const htmlEl = root.children.find(e => e.tag === 'html');
if (!htmlEl) { console.error('no <html> element in ' + domFile); process.exit(1); }
walk(htmlEl, '');
const keys = fs.readFileSync(keysFile, 'utf8').split('\n').filter(l => l.trim());
for (const k of keys) console.log(`${have.has(k) ? 'RAW' : 'JS_ONLY'}\t${k}`);
