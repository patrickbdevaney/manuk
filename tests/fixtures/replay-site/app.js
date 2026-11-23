fetch('data.json').then(r=>r.json()).then(d=>{document.getElementById('out').textContent='api says '+d.msg+' '+d.items.join(',');});
document.getElementById('p').textContent='script ran';
