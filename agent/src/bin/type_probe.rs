//! `type-probe <url> <textbox-name-or-empty> <text> [css-selector]` — W4.P0.b. Loads the page, types `text`
//! into the named text box through the path MCP `act type` takes (AgentBrowser::type_into_handle), settles,
//! and prints the field's value, the page's listener trail (keydown/input/change on the field) and the
//! visible text after typing, so a typeahead list can be told present or absent.
use anyhow::{bail, Result};
use manuk_agent::AgentBrowser;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        bail!("usage: type-probe <url> <textbox-name> <text> [wait-ms]");
    }
    let mut b = AgentBrowser::new(1280, 900);
    b.navigate(&args[0]).await?;
    b.settle().await;
    let before = b.a11y_tree()?;
    let boxes: Vec<String> = before
        .iter()
        .filter(|n| n.role == manuk_a11y::Role::ComboBox)
        .map(|n| format!("{:?}", n.name))
        .collect();
    println!("textboxes: {boxes:?}");
    let node = b.resolve_handle(&manuk_a11y::Role::ComboBox, &args[1])?;
    b.probe_eval(
        r#"(function(){window.__tlog=[];document.addEventListener('input',function(e){var k=Object.getOwnPropertyNames(e.target).filter(function(k){return k.indexOf('__reactProps')==0;})[0];window.__tlog.push('AFTER-react-input node.v='+e.target.value+' prop.v='+(k&&e.target[k].value));},false);var in0=document.querySelector('input[placeholder="Enter Location"]');var k0=Object.getOwnPropertyNames(in0).filter(function(k){return k.indexOf('__reactProps')==0;})[0];var p0=in0[k0];var oc=p0.onChange;p0.onChange=function(e){window.__tlog.push('REACT onChange v='+(e&&e.target&&e.target.value));return oc.apply(this,arguments);};['keydown','input','change','focus','blur','focusin','focusout','click','mousedown'].forEach(function(t){document.addEventListener(t,function(e){window.__tlog.push(t+' '+(e.target&&e.target.tagName)+' v='+(e.target&&e.target.value)+' d='+(e.data||e.key||'')+' it='+e.inputType);},true);});})()"#,
    );
    if std::env::var("TYPE_PROBE_JS").is_ok() {
        // Control: the page-JS way (native value setter + bubbling input event), as a browser's own typing would.
        b.focus_handle(node)?;
        let (_, e) = b.probe_eval(&format!("(function(){{var el=document.querySelector('input[aria-label=\"Enter Location\"],input[placeholder]');el.focus();var set=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set;set.call(el,{:?});el.dispatchEvent(new Event('input',{{bubbles:true}}));}})()", args[2]));
        println!("js-errs: {e:?}");
    } else {
        b.type_into_handle(node, &args[2])?;
    }
    b.settle().await;
    let wait: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1500);
    tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
    b.settle().await;
    let (out, errs) = b.probe_eval(
        "(function(){var p=document.createElement('pre');p.id='__probe';document.body.appendChild(p);var ae=document.activeElement;var pk=ae&&Object.getOwnPropertyNames(ae).filter(function(k){return k.indexOf('__reactProps')==0;})[0];var pr=pk&&ae[pk];var in1=document.querySelector('input[placeholder^=Enter]');var k1=Object.getOwnPropertyNames(in1).filter(function(k){return k.indexOf('__reactProps')==0;})[0];var opts=[].map.call(document.querySelectorAll('[role=option]'),function(o){return o.textContent.slice(0,20);});window.__fin={nodeV:in1.value,attrV:in1.getAttribute('value'),propV:in1[k1].value,nopts:opts.length,opts:opts.slice(0,3)};window.__pr=pr?Object.keys(pr).map(function(k){return k+':'+typeof pr[k]+(typeof pr[k]=='string'?'='+pr[k]:'');}):'noprops';p.textContent=JSON.stringify({fin:window.__fin,type:ae&&ae.type,events:window.__tlog.length,last:window.__tlog.slice(-14),active:ae&&ae.tagName+' '+(ae.value||''),text:document.body.innerText.slice(0,900)});})()",
    );
    println!("{out}");
    for e in errs.iter().take(10) {
        println!("boot-error: {e}");
    }
    Ok(())
}
