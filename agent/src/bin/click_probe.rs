//! `click-probe <url> <css-selector> [engine <image-name>]` — W4.P0.a. Loads the page, installs capture/bubble
//! click recorders on window and document, dispatches a click on the selected element through the
//! page's JS, and prints the dispatch trail (which phases ran, which ancestors carry React props with
//! an `onClick`), the URL and any boot errors. The trail is read back through a `#__probe` element.
use anyhow::{bail, Result};
use manuk_agent::AgentBrowser;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        bail!("usage: click-probe <url> <css-selector> [wait-ms]");
    }
    let mut b = AgentBrowser::new(1280, 900);
    b.navigate(&args[0]).await?;
    b.settle().await;
    let sel = args[1].replace('\\', "\\\\").replace('\'', "\\'");
    let src = format!(
        r#"(function(){{
var log=[];var el=document.querySelector('{sel}');
if(!el){{var p=document.createElement('pre');p.id='__probe';p.textContent='NOEL';document.body.appendChild(p);return;}}
function d(n){{return n&&n.tagName?n.tagName.toLowerCase()+(n.id?'#'+n.id:''):String(n);}}
var chain=[];for(var n=el;n;n=n.parentNode){{
 var ks=Object.getOwnPropertyNames(n).filter(function(k){{return k.indexOf('__react')==0;}});
 var pk=ks.filter(function(k){{return k.indexOf('__reactProps')==0;}})[0];
 var oc=pk&&n[pk]&&typeof n[pk].onClick;
 chain.push(d(n)+(ks.length?'['+ks.length+'rk'+(oc?',onClick='+oc:'')+']':'')+(n.onclick?'[onclick]':''));
}}
window.addEventListener('click',function(e){{log.push('win-cap target='+d(e.target));}},true);
document.addEventListener('click',function(e){{log.push('doc-cap');}},true);
document.addEventListener('click',function(e){{log.push('doc-bub');}},false);
window.addEventListener('click',function(e){{log.push('win-bub');}},false);
var before=location.href;
var ev=new MouseEvent('click',{{bubbles:true,cancelable:true,view:window}});
var r=el.dispatchEvent(ev);
var p=document.createElement('pre');p.id='__probe';document.body.appendChild(p);p.textContent=JSON.stringify({{chain:chain,log:log,notCancelled:r,before:before,after:location.href}});
}})()"#
    );
    let (out, errs) = if args.get(2).map(String::as_str) == Some("engine") {
        // The path the MCP `act click` takes: AgentBrowser::activate -> Page::dispatch_click.
        let name = args[3].clone();
        let node = b.resolve_handle(&manuk_a11y::Role::Image, &name)?;
        b.probe_eval(
            r#"(function(){var log=[];window.__plog=log;
var of=window.fetch;window.fetch=function(u){log.push('fetch '+(u&&u.url||u));return of.apply(this,arguments).then(function(r){log.push('fetched '+r.status+' '+(u&&u.url||u));return r;},function(e){log.push('fetchfail '+e);throw e;});};window.addEventListener('error',function(e){log.push('err '+e.message);});window.addEventListener('unhandledrejection',function(e){log.push('rej '+(e.reason&&e.reason.message||e.reason));});var ps=history.pushState;history.pushState=function(){log.push('pushState '+arguments[2]);return ps.apply(this,arguments);};
document.addEventListener('click',function(e){log.push('doc-cap target='+(e.target&&e.target.tagName)+' x='+e.clientX+' y='+e.clientY+' trusted='+e.isTrusted);},true);
document.addEventListener('click',function(e){log.push('doc-bub');},false);})()"#,
        );
        let act = b.activate(node).await?;
        if args.get(4).is_some() {
            b.settle().await;
            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            b.settle().await;
        }
        let (o, e) = b.probe_eval(
            "(function(){var p=document.createElement('pre');p.id='__probe';document.body.appendChild(p);p.textContent=JSON.stringify({log:window.__plog,href:location.href,rt:(window.next&&next.router?next.router.pathname+' ready='+next.router.isReady:'nonext'),pep:document.body.innerText.indexOf('Pepperoni'),len:document.body.innerText.length,next:(document.getElementById('__next')||{}).innerText.slice(300,700)});})()",
        );
        println!("activation={act:?} script_navigation={:?}", b.probe_take_script_navigation());
        (o, e)
    } else {
        b.probe_eval(&src)
    };
    println!("{out}");
    for e in errs.iter().take(10) {
        println!("boot-error: {e}");
    }
    Ok(())
}
