// W3.20: the WebMCP consumer bridge. Installed on every new document (before page script) so each
// registerTool / provideContext call is captured, whether or not the browser ships a native
// modelContext. `__manukWebmcp.list()` is what the page offers (registered + declarative + one
// synthesized fill_form tool per <form>); `.call(name, args)` runs one.
(() => {
  if (window.__manukWebmcp) return;
  const reg = new Map();
  const hook = (mc) => {
    const o = { r: mc.registerTool, u: mc.unregisterTool, p: mc.provideContext, c: mc.clearContext };
    const put = (t) => { if (t && typeof t.name === 'string') reg.set(t.name, t); };
    mc.registerTool = function (t) { put(t); return o.r ? o.r.apply(this, arguments) : undefined; };
    mc.unregisterTool = function (n) { reg.delete(typeof n === 'string' ? n : n && n.name); return o.u ? o.u.apply(this, arguments) : undefined; };
    mc.provideContext = function (c) { reg.clear(); ((c && c.tools) || []).forEach(put); return o.p ? o.p.apply(this, arguments) : undefined; };
    mc.clearContext = function () { reg.clear(); return o.c ? o.c.apply(this, arguments) : undefined; };
  };
  let mc = navigator.modelContext;
  if (mc) hook(mc);
  else {
    mc = {};
    hook(mc);
    try { Object.defineProperty(navigator, 'modelContext', { value: mc, configurable: true }); } catch (e) {}
  }
  // The consumer-facing spelling is document.modelContext; both name the same object.
  try { if (!document.modelContext) Object.defineProperty(document, 'modelContext', { value: mc, configurable: true }); } catch (e) {}

  const ZW = /[​-‏⁠⁢-⁤﻿]|[\u{e0000}-\u{e007f}]/u;
  const unseen = (el) => {
    for (let e = el; e && e.nodeType === 1; e = e.parentElement) {
      const s = getComputedStyle(e);
      if (s.display === 'none' || s.visibility === 'hidden' || s.visibility === 'collapse') return true;
      if (parseFloat(s.opacity) < 0.05) return true;
      if (parseFloat(s.fontSize) < 2) return true;
    }
    const r = el.getBoundingClientRect();
    if (r.width < 2 || r.height < 2) return true;
    if (r.right < -100 || r.bottom < -100 || r.left > innerWidth + 5000 || r.top > document.documentElement.scrollHeight + 5000) return true;
    return false;
  };
  // Text a human cannot see (paint truth), lowercased.
  const hiddenTexts = () => {
    const out = [];
    const w = document.createTreeWalker(document.body || document.documentElement, NodeFilter.SHOW_TEXT);
    for (let n; (n = w.nextNode());) {
      const t = (n.nodeValue || '').trim();
      const p = n.parentElement;
      if (t.length >= 8 && p && !/^(SCRIPT|STYLE|NOSCRIPT|TEMPLATE)$/.test(p.tagName) && unseen(p)) out.push(t.toLowerCase());
    }
    return out;
  };
  const taintOf = (desc, hidden) => {
    const d = String(desc || '');
    if (ZW.test(d)) return 'invisible characters in the description';
    const dl = d.toLowerCase();
    for (const h of hidden) if (dl.includes(h) || (h.length > 20 && h.includes(dl.trim()) && dl.trim().length > 8)) return 'description holds text hidden on the page: "' + h.slice(0, 60) + '"';
    return '';
  };

  const labelOf = (f) => {
    if (f.getAttribute('aria-label')) return f.getAttribute('aria-label');
    if (f.id) { const l = document.querySelector('label[for="' + CSS.escape(f.id) + '"]'); if (l) return l.textContent.trim(); }
    const l = f.closest('label'); if (l) return l.textContent.trim();
    return f.getAttribute('placeholder') || f.name || f.id || '';
  };
  const formFields = (form) => [...form.elements].filter((f) => /^(INPUT|SELECT|TEXTAREA)$/.test(f.tagName) && !/^(submit|button|reset|image|hidden)$/.test(f.type || '') && (f.name || f.id) && !f.disabled);
  const keyOf = (f) => f.name || f.id;
  const formSchema = (form) => {
    const properties = {}, required = [], seen = new Set();
    for (const f of formFields(form)) {
      const k = keyOf(f);
      if (seen.has(k)) { if (f.type === 'radio') properties[k].enum.push(f.value); continue; }
      seen.add(k);
      let p;
      if (f.tagName === 'SELECT') p = { type: 'string', enum: [...f.options].map((o) => o.value) };
      else if (f.type === 'checkbox') p = { type: 'boolean' };
      else if (f.type === 'radio') p = { type: 'string', enum: [f.value] };
      else if (f.type === 'number' || f.type === 'range') p = { type: 'number' };
      else p = { type: 'string' };
      const l = labelOf(f); if (l) p.description = l;
      properties[k] = p;
      if (f.required) required.push(k);
    }
    return { type: 'object', properties, required };
  };
  const forms = () => [...document.forms];
  const formName = (form, i) => form.getAttribute('toolname') || ('fill_form_' + (form.id || form.getAttribute('name') || i));

  window.__manukWebmcp = {
    list() {
      const hidden = hiddenTexts();
      const tools = [];
      const push = (name, description, inputSchema, source, annotations) => {
        const why = taintOf(description, hidden) || taintOf(name, hidden);
        tools.push({ name, description: why ? '' : description, inputSchema, source, annotations: annotations || {}, withheld: why || undefined });
      };
      for (const t of reg.values()) {
        let schema = t.inputSchema;
        if (typeof schema === 'string') { try { schema = JSON.parse(schema); } catch (e) { schema = null; } }
        push(t.name, String(t.description || ''), schema || { type: 'object', properties: {} }, 'registerTool', t.annotations);
      }
      forms().forEach((form, i) => {
        const name = formName(form, i);
        if (reg.has(name)) return;
        const declared = form.hasAttribute('toolname');
        const schema = formSchema(form);
        if (!Object.keys(schema.properties).length) return;
        schema.properties.submit = { type: 'boolean', description: 'also submit the form after filling (default false)' };
        push(name, declared ? (form.getAttribute('tooldescription') || 'Fill and submit the form') : ('Fill the fields of the form' + (form.id ? ' #' + form.id : ' #' + (i + 1)) + (form.getAttribute('action') ? ' (action ' + form.getAttribute('action') + ')' : '')), schema, declared ? 'declarative' : 'synthesized', {});
      });
      return tools;
    },
    async call(name, args) {
      args = args || {};
      const t = reg.get(name);
      if (t) {
        const r = await t.execute(args, { requestUserInteraction: async (cb) => cb() });
        return typeof r === 'string' ? r : JSON.stringify(r === undefined ? null : r);
      }
      const fs = forms();
      const i = fs.findIndex((f, j) => formName(f, j) === name);
      if (i < 0) throw new Error('no such WebMCP tool: ' + name);
      const form = fs[i];
      const done = [], missing = [];
      for (const [k, v] of Object.entries(args)) {
        if (k === 'submit') continue;
        const els = formFields(form).filter((f) => keyOf(f) === k);
        if (!els.length) { missing.push(k); continue; }
        const f = els[0];
        const fire = (el) => { el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true })); };
        if (f.type === 'radio') { const r = els.find((e) => e.value === String(v)); if (!r) { missing.push(k + '=' + v); continue; } r.checked = true; fire(r); }
        else if (f.type === 'checkbox') { f.checked = v === true || v === 'true'; fire(f); }
        else if (f.tagName === 'SELECT') { f.value = String(v); if (f.value !== String(v)) { missing.push(k + '=' + v); continue; } fire(f); }
        else {
          // The native setter keeps React-style value trackers honest; an engine whose `value` is not an
          // accessor on the prototype just takes the assignment.
          const d = Object.getOwnPropertyDescriptor(f.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype, 'value');
          if (d && d.set) d.set.call(f, String(v)); else f.value = String(v);
          fire(f);
        }
        done.push(k);
      }
      let sub = '';
      if (args.submit === true) { if (form.requestSubmit) form.requestSubmit(); else form.submit(); sub = '; submitted'; }
      return 'filled ' + done.length + ' field(s) [' + done.join(', ') + ']' + (missing.length ? '; could not set [' + missing.join(', ') + ']' : '') + sub;
    },
  };
})();
