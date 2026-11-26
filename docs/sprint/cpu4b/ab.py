#!/usr/bin/env python3
"""CPU.4.b A/B: ab.py <old-bin-dir> <new-bin-dir> <rounds> <out.tsv>. Interleaved probe.py runs of manuk-mcp on w3c (navigate+observe); one row per run:
round arm total_wait4 tokio_worker_cpu n_images image_sizes(sorted) a11y_sha (observe text with the loopback port masked) chrome_ver."""
import subprocess,sys,os,re,json,hashlib
old,new,rounds,out=sys.argv[1],sys.argv[2],int(sys.argv[3]),sys.argv[4]
here=os.path.dirname(os.path.abspath(__file__))
with open(out,"a") as f:
    for r in range(rounds):
        for arm,d in (("old",old),("new",new)) if r%2==0 else (("new",new),("old",old)):
            env=dict(os.environ,MANUK_BIN_DIR=d)
            o=subprocess.run([sys.executable,here+"/probe.py","mcp"],env=env,capture_output=True,text=True).stdout
            tot=float(re.search(r"total_wait4 ([\d.]+)",o).group(1))
            tw=sum(float(x) for x in re.findall(r"tokio-rt-worker\t([\d.]+)",o))
            err=open("/tmp/cpu4b.err").read()
            imgs=sorted(re.findall(r"IMGLOG \d+ (\d+x\d+) (\S+)",err))
            txt=json.load(open("/tmp/cpu4b.obs"))["result"]["content"][0]["text"]
            txt=re.sub(r"127\.0\.0\.1:\d+","H",txt)
            f.write("\t".join(map(str,[r,arm,tot,tw,len(imgs),";".join(a+" "+b.rsplit("/",1)[1] for a,b in imgs),hashlib.sha256(txt.encode()).hexdigest()[:12],"145.0.7632.116"]))+"\n");f.flush()
