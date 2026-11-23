// W3.20.b.b: page-level trace of who registers book_table_le_petit_bistro on french-bistro, in the pinned Chrome
// (native, WebMCP+WebMCPTesting, document.modelContext alias as in compare.mjs). Wraps registerTool BEFORE page
// script, logs each call with a stack and its settle/reject outcome. Usage: node trace-bistro.mjs
import { spawn } from 'node:child_process';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
const CHROME = process.env.MANUK_CHROME || '/home/patrickd/.local/chrome-cft/145.0.7632.116/chrome-linux64/chrome.real';
const profile = mkdtempSync(join(tmpdir(), 'bistro-'));
const ch = spawn(CHROME, ['--headless=new','--no-sandbox','--disable-gpu','--password-store=basic',`--user-data-dir=${profile}`,'--enable-features=WebMCP,WebMCPTesting','--remote-debugging-pipe','about:blank'],{stdio:['ignore','ignore','ignore','pipe','pipe']});
let id=1,buf='';const pend=new Map();
ch.stdio[4].setEncoding('utf8');
ch.stdio[4].on('data',c=>{buf+=c;let i;while((i=buf.indexOf('\0'))>=0){const m=JSON.parse(buf.slice(0,i));buf=buf.slice(i+1);if(m.id&&pend.has(m.id)){pend.get(m.id)(m);pend.delete(m.id);}}});
const cdp=(method,params={},sessionId)=>new Promise((res,rej)=>{const i=id++;pend.set(i,m=>m.error?rej(new Error(JSON.stringify(m.error))):res(m.result));const o={id:i,method,params};if(sessionId)o.sessionId=sessionId;ch.stdio[3].write(JSON.stringify(o)+'\0');});
const PRE=`(() => {
  const mc = navigator.modelContext; if (!mc) return;
  if (!document.modelContext) Object.defineProperty(document,'modelContext',{value:mc,configurable:true});
  window.__trace = [];
  const o = mc.registerTool;
  mc.registerTool = function (t) {
    const e = { name: t && t.name, stack: (new Error().stack||'').split('\\n').slice(2,5).map(s=>s.trim()) };
    window.__trace.push(e);
    let r; try { r = o.apply(this, arguments); } catch (x) { e.threw = String(x); throw x; }
    Promise.resolve(r).then(()=>{e.outcome='resolved'},x=>{e.outcome='rejected: '+x});
    return r;
  };
})()`;
const {targetId}=await cdp('Target.createTarget',{url:'about:blank'});
const {sessionId}=await cdp('Target.attachToTarget',{targetId,flatten:true});
await cdp('Page.enable',{},sessionId);
await cdp('Page.addScriptToEvaluateOnNewDocument',{source:PRE},sessionId);
await cdp('Page.navigate',{url:'https://googlechromelabs.github.io/webmcp-tools/demos/french-bistro/'},sessionId);
await new Promise(r=>setTimeout(r,6000));
const r=await cdp('Runtime.evaluate',{expression:`(async()=>JSON.stringify({trace:window.__trace,native:await navigator.modelContextTesting.listTools(), forms:[...document.forms].map(f=>f.getAttribute('toolname'))}))()`,awaitPromise:true,returnByValue:true},sessionId);
console.log(r.result.value);
ch.kill();process.exit(0);
