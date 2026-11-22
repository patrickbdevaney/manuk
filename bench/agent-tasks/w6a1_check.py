#!/usr/bin/env python3
"""W6.A1 integrity + agreement. usage: w6a1_check.py merge LABELS_DIR OUT.tsv | verify OUT.tsv ROWS.tsv | agree OUT.tsv SECOND.tsv | self-test
verify: every failed run of the 3 arms present exactly once, one allowed cause each. agree: prints k/n; exit 1 if <9/10 (n=10)."""
import sys,csv,glob,collections
CAUSES=set("wrong_answer dialog_modal widget_unreachable stopped_early app_error typed_not_registered no_final_answer click_no_effect key_hover_enter_gap handle_lost payment_rejected task_data_missing never_reached_page tool_surface_gap state_not_persisted declined_action".split())
CV='145.0.7632.116'
def rd(p): return list(csv.DictReader(open(p),delimiter='\t'))
def verify(lab,rows):
    keys=[(r['arm'],r['task'],r['run']) for r in rows]
    got=[(r['arm'],r['task'],r['run']) for r in lab]
    err=[]
    if len(got)!=len(set(got)): err.append('duplicate rows')
    if set(got)!=set(keys): err.append(f'key mismatch: missing {len(set(keys)-set(got))} extra {len(set(got)-set(keys))}')
    for r in lab:
        if r['cause'] not in CAUSES: err.append('bad cause '+repr(r['cause']))
        if r.get('chrome_ver')!=CV: err.append('chrome_ver')
    return err
def agree(lab,sec):
    L={(r['arm'],r['task'],r['run']):r['cause'] for r in lab}
    k=0;n=0
    for r in sec:
        n+=1;k+= L.get((r['arm'],r['task'],r['run']))==r['cause']
    return k,n
def main():
    a=sys.argv[1:]
    if a[0]=='merge':
        out=[]
        for f in sorted(glob.glob(a[1]+'/labels-*.tsv')): out+=rd(f)
        with open(a[2],'w') as f:
            f.write('arm\ttask\trun\tcause\tbasis\tchrome_ver\n')
            for r in out: f.write('\t'.join([r['arm'],r['task'],r['run'].lstrip('r'),r['cause'],r['basis'].replace('\t',' '),CV])+'\n')
        print(len(out))
    elif a[0]=='verify':
        e=verify(rd(a[1]),rd(a[2])); print(e or 'OK'); sys.exit(1 if e else 0)
    elif a[0]=='agree':
        k,n=agree(rd(a[1]),rd(a[2])); print(f'{k}/{n}'); sys.exit(0 if k*10>=9*n else 1)
    elif a[0]=='self-test':
        rows=[{'arm':'m','task':'t','run':str(i)} for i in range(10)]
        lab=[{**r,'cause':'wrong_answer','chrome_ver':CV} for r in rows]
        assert not verify(lab,rows)
        assert verify(lab[:-1],rows)                       # dropped row
        assert verify(lab+[lab[0]],rows)                   # duplicate
        assert verify([{**lab[0],'cause':'bogus'}]+lab[1:],rows)
        assert agree(lab,lab)==(10,10)
        sec=[{**r,'cause':'dialog_modal'} for r in lab[:2]]+lab[2:]
        assert agree(lab,sec)==(8,10)                      # flipped 2 -> red
        print('self-test GREEN')
main()
