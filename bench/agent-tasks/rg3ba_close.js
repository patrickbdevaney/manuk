(function(){
  var out=[], all=document.querySelectorAll('*');
  function nm(e){return ((e.getAttribute('aria-label')||'')+'|'+(e.getAttribute('title')||'')+'|'+((e.children.length==0?e.textContent:'')||'')).toLowerCase();}
  function d(e){
    if(!e) return null;
    var cs=getComputedStyle(e), r=e.getBoundingClientRect();
    return {tag:e.tagName.toLowerCase(), cls:String(e.getAttribute('class')||'').slice(0,60), role:e.getAttribute('role')||'',
      aria:e.getAttribute('aria-label')||'', text:(e.textContent||'').trim().slice(0,20),
      display:cs.display, visibility:cs.visibility, opacity:cs.opacity, position:cs.position,
      rect:[Math.round(r.x),Math.round(r.y),Math.round(r.width),Math.round(r.height)]};
  }
  for(var i=0;i<all.length;i++){
    var e=all[i], t=e.tagName.toLowerCase();
    if(t=='script'||t=='style'||t=='head'||t=='html') continue;
    var isbtn=(t=='button'||e.getAttribute('role')=='button');
    if(isbtn && /(^|\|)\s*close\s*(\||$)/.test(nm(e))) out.push({el:d(e),parent:d(e.parentElement)});
  }
  var b=(document.body&&document.body.innerText)||'';
  return JSON.stringify({n:all.length,banner_text:/Version 2\.0 launches soon/.test(b),alerts:document.querySelectorAll('[role=alert]').length,hits:out});
})()
