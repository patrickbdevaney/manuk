//! `click-probe <url> <css-selector | text=Button label> [engine <image-name>]` — W4.P0.a. Loads the page, installs capture/bubble
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
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    b.settle().await;
    let sel = args[1].replace('\\', "\\\\").replace('\'', "\\'");
    let src = format!(
        r#"(function(){{
var log=[];var el=('{sel}'.indexOf('text=')==0)?Array.prototype.filter.call(document.querySelectorAll('button,a,[role=button],div,span,p'),function(x){{return x.children.length==0&&x.textContent.indexOf('{sel}'.slice(5))>=0;}})[0]:document.querySelector('{sel}');
if(!el){{var p=document.createElement('pre');p.id='__probe';p.textContent='NOEL len='+document.body.innerText.length+' '+document.body.innerText.slice(0,200);document.body.appendChild(p);return;}}
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
    let (out, errs) = if args.get(2).map(String::as_str) == Some("seq") {
        // `seq` mode: args[3..] are `Role:Name` clicks run through the engine path in order (Role = button|textbox);
        // afterwards the selected element's React/DOM state is dumped.
        for st in &args[3..] {
            let (r, n) = st.split_once(':').unwrap_or(("button", st));
            if r == "type" {
                // `type:<textbox name>=<text>`
                let (nm, text) = n.split_once('=').unwrap_or((n, ""));
                let node = b.resolve_handle(&manuk_a11y::Role::TextBox, nm)?;
                b.type_into_handle(node, text)?;
                b.settle().await;
                continue;
            }
            let role = if r == "textbox" {
                manuk_a11y::Role::TextBox
            } else {
                manuk_a11y::Role::Button
            };
            if let Ok(pre) = std::env::var("PROBE_PRE_JS") {
                b.probe_eval(&pre);
            }
            // `Name#K`: the K-th (0-based) match of that name in document order.
            let act = match n
                .rsplit_once('#')
                .and_then(|(nm, k)| Some((nm, k.parse::<usize>().ok()?)))
            {
                Some((nm, k)) => {
                    let tree = b.a11y_tree()?;
                    let node = tree
                        .iter()
                        .filter(|x| x.role.matches(&role) && x.name == nm)
                        .nth(k)
                        .map(|x| x.node)
                        .ok_or_else(|| anyhow::anyhow!("no {nm}#{k}"))?;
                    b.activate(node).await
                }
                None => b.click_by_name(&role, n).await,
            };
            b.settle().await;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            println!(
                "{st}: {act:?} scroll_y={} nav={:?}",
                b.scroll_offset(),
                b.probe_take_script_navigation()
            );
        }
        if let Ok(js) = std::env::var("PROBE_JS") {
            // `PROBE_JS`: a function body returning the string to print, run after the clicks.
            b.probe_eval(&format!(
                "(function(){{var p=document.createElement('pre');p.id='__probe';document.body.appendChild(p);p.textContent=String((function(){{{js}}})());}})()"
            ))
        } else {
            b.probe_eval(&format!(
            r#"(function(){{var el=document.querySelector('{sel}');var p=document.createElement('pre');p.id='__probe';document.body.appendChild(p);
if(!el){{p.textContent='NOEL';return;}}var ks=Object.getOwnPropertyNames(el).filter(function(k){{return k.indexOf('__reactProps')==0;}});var pr=ks.length?el[ks[0]]:{{}};
var r=el.getBoundingClientRect();var c=getComputedStyle(el);var top=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);
p.textContent=JSON.stringify({{onClick:typeof pr.onClick,onclickAttr:typeof el.onclick,disabled:el.disabled,inert:el.hasAttribute('inert')||!!el.closest('[inert]'),pe:c.pointerEvents,vis:c.visibility,op:c.opacity,rect:[r.x,r.y,r.width,r.height],top:top===el?'SELF':(top&&top.tagName+':'+(top.getAttribute('aria-label')||top.className)),connected:el.isConnected,sy:window.scrollY,props:Object.keys(pr)}});}})()"#
        ))
        }
    } else if args.get(2).map(String::as_str) == Some("at") {
        // `at` mode: scrollY=args[3], point (args[4], args[5]); lists elementsFromPoint with paint-relevant styles.
        let (dy, x, y) = (&args[3], &args[4], &args[5]);
        b.probe_eval(&format!(
            r#"(function(){{window.scrollTo(0,{dy});var p=document.createElement('pre');p.id='__probe';document.body.appendChild(p);
function d(n){{var c=getComputedStyle(n);var r=n.getBoundingClientRect();return n.tagName.toLowerCase()+'.'+String(n.className).slice(0,40)+' bg='+c.backgroundColor+' op='+c.opacity+' pos='+c.position+' z='+c.zIndex+' disp='+c.display+' pe='+c.pointerEvents+' filt='+c.filter+' r='+[r.x,r.y,r.width,r.height].map(Math.round);}}
p.textContent=JSON.stringify(document.elementsFromPoint({x},{y}).slice(0,10).map(d),null,1);}})()"#
        ))
    } else if args.get(2).map(String::as_str) == Some("geom") {
        // `geom` mode: where the selected element is and what elementsFromPoint says covers its centre, at scrollY = args[3].
        let dy = args.get(3).map(String::as_str).unwrap_or("0");
        b.probe_eval(&format!(
            r#"(function(){{window.scrollTo(0,{dy});var el=document.querySelector('{sel}');var p=document.createElement('pre');p.id='__probe';document.body.appendChild(p);
if(!el){{p.textContent='NOEL';return;}}var r=el.getBoundingClientRect();var cx=r.x+r.width/2,cy=r.y+r.height/2;
function d(n){{var c=getComputedStyle(n);return n.tagName.toLowerCase()+(n.getAttribute('aria-label')?'['+n.getAttribute('aria-label')+']':'')+'{{pos='+c.position+',z='+c.zIndex+',pe='+c.pointerEvents+'}}';}}
p.textContent=JSON.stringify({{sy:window.scrollY,rect:[r.x,r.y,r.width,r.height],efp:document.elementFromPoint(cx,cy)===el?'SELF':d(document.elementFromPoint(cx,cy)),all:document.elementsFromPoint(cx,cy).slice(0,8).map(d),parents:(function(){{var o=[];for(var n=el.parentElement;n&&o.length<7;n=n.parentElement)o.push(d(n));return o;}})()}});}})()"#
        ))
    } else if args.get(2).map(String::as_str) == Some("engine") {
        // The path the MCP `act click` takes: AgentBrowser::activate -> Page::dispatch_click.
        let name = args[3].clone();
        let role = if std::env::var("PROBE_ROLE").as_deref() == Ok("button") {
            manuk_a11y::Role::Button
        } else {
            manuk_a11y::Role::Image
        };
        let node = b.resolve_handle(&role, &name)?;
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
        println!(
            "activation={act:?} script_navigation={:?}",
            b.probe_take_script_navigation()
        );
        (o, e)
    } else {
        // Record fetches, errors and history writes that the click triggers, then read them after the page settles.
        b.probe_eval(
            r#"(function(){var log=[];window.__plog=log;var of=window.fetch;window.fetch=function(u){log.push('fetch '+(u&&u.url||u));return of.apply(this,arguments).then(function(r){log.push('fetched '+r.status+' '+(u&&u.url||u));return r;},function(e){log.push('fetchfail '+e);throw e;});};window.addEventListener('error',function(e){log.push('err '+e.message);});window.addEventListener('unhandledrejection',function(e){log.push('rej '+(e.reason&&e.reason.message||e.reason));});var ps=history.pushState;history.pushState=function(){log.push('pushState '+arguments[2]);return ps.apply(this,arguments);};var rs=history.replaceState;history.replaceState=function(){log.push('replaceState '+arguments[2]);return rs.apply(this,arguments);};})()"#,
        );
        let r = b.probe_eval(&src);
        b.settle().await;
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        b.settle().await;
        let (o, _) = b.probe_eval(
            "(function(){var q=document.getElementById('__probe');if(q)q.remove();var p=document.createElement('pre');p.id='__probe';document.body.appendChild(p);p.textContent=JSON.stringify({log:window.__plog,href:location.href});})()",
        );
        println!("{}", r.0);
        (o, r.1)
    };
    println!("{out}");
    for e in errs.iter().take(10) {
        println!("boot-error: {e}");
    }
    Ok(())
}
