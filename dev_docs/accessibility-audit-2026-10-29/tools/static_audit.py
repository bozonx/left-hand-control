#!/usr/bin/env python3
"""Reproducible source inventory + WCAG sRGB pair calculations (not a UI certification)."""
import argparse, csv, hashlib, json, re
from pathlib import Path
p=argparse.ArgumentParser(); p.add_argument('--root',type=Path,default=Path.cwd()); p.add_argument('--out',type=Path,required=True); a=p.parse_args(); a.out.mkdir(parents=True,exist_ok=True)
files=sorted((a.root/'apps/slint-shell/ui').glob('*.slint'))
# Mask comments and quoted strings while preserving offsets/newlines.
def mask(s):
    return re.sub(r'//[^\n]*|/\*[\s\S]*?\*/|"(?:\\.|[^"\\])*"',lambda m: ''.join('\n' if c=='\n' else ' ' for c in m[0]),s)
entries=[]; inventory=[]
for path in files:
    s=path.read_text(); m=mask(s); stack=[]; ends={}
    for i,c in enumerate(m):
        if c=='{': stack.append(i)
        elif c=='}' and stack: ends[stack.pop()]=i
    counts={k:len(re.findall(v,m)) for k,v in {'touch':r'\bTouchArea\s*\{','focus':r'\bFocusScope\s*\{','accessible_roles':r'accessible-role\s*:','keyboard_handlers':r'(?:capture-)?key-pressed\s*\(','live_regions':r'accessible-live-region\s*:','text_fields':r'\b(?:TextField|SearchField|TextEdit|SelectField|ComboBox|NumericInput|DurationInput)\s*\{'}.items()}
    inventory.append({'file':str(path.relative_to(a.root)),'lines':len(s.splitlines()),'sha256':hashlib.sha256(s.encode()).hexdigest(),**counts})
    for hit in re.finditer(r'\b(TouchArea|FocusScope|TextField|SearchField|TextEdit|SelectField|ComboBox|NumericInput|DurationInput|FieldButton|IconButton|Button|PopupWindow|DialogFrame|ConfirmDialog)\s*\{',m):
        start=m.find('{',hit.start()); end=ends.get(start)
        if end is None: continue
        # Direct properties only, not properties of descendants.
        direct=list(s[start+1:end]); depth=0
        for j,c in enumerate(m[start+1:end]):
            if c=='{': depth+=1
            if depth>0: direct[j]='\n' if c=='\n' else ' '
            if c=='}': depth-=1
        direct=''.join(direct)
        props={k: bool(re.search(r'\b'+re.escape(k)+r'\s*(?::|<=>|\()',direct)) for k in ['accessible-label','accessible-role','accessible-description','accessible-enabled','accessible-checked','accessible-item-selected','accessible-delegate-focus','key-pressed','capture-key-pressed','opacity','height','width']}
        entries.append({'file':str(path.relative_to(a.root)),'line':s.count('\n',0,hit.start())+1,'type':hit[1],**props,'direct':direct.strip()})
(a.out/'source-inventory.json').write_text(json.dumps({'files':inventory,'instances':entries},ensure_ascii=False,indent=2)+'\n')
# Parse the actual ternary expressions of Theme, in declaration order.
theme=(a.root/'apps/slint-shell/ui/theme.slint').read_text()
expressions=re.findall(r'(?:out )?property <color> ([\w-]+): ([^;]+);',theme)
def evaluate(expr,env):
    tokens=re.findall(r'#[0-9a-fA-F]+|[\w-]+|\|\||&&|[()?:!]',expr); n=0
    def ternary():
        nonlocal n
        v=or_expr()
        if n<len(tokens) and tokens[n]=='?':
            n+=1; yes=ternary(); assert tokens[n]==':'; n+=1; no=ternary(); return yes if v else no
        return v
    def or_expr():
        nonlocal n
        v=and_expr()
        while n<len(tokens) and tokens[n]=='||': n+=1; rhs=and_expr(); v=v or rhs
        return v
    def and_expr():
        nonlocal n
        v=atom()
        while n<len(tokens) and tokens[n]=='&&': n+=1; rhs=atom(); v=v and rhs
        return v
    def atom():
        nonlocal n
        t=tokens[n]; n+=1
        if t=='!': return not atom()
        if t=='(': v=ternary(); assert tokens[n]==')'; n+=1; return v
        if t.startswith('#'): return t
        return env[t]
    result=ternary(); assert n==len(tokens),(expr,tokens[n:]); return result

def rgb(h):return tuple(int(h[i:i+2],16)/255 for i in (1,3,5))
def lum(h):
    r,g,b=rgb(h); conv=lambda x:x/12.92 if x<=0.04045 else ((x+0.055)/1.055)**2.4
    return .2126*conv(r)+.7152*conv(g)+.0722*conv(b)
def ratio(f,b):return (max(lum(f),lum(b))+.05)/(min(lum(f),lum(b))+.05)
def blend(f,b,opacity):return '#'+''.join(f'{round((x*opacity+y*(1-opacity))*255):02x}' for x,y in zip(rgb(f),rgb(b)))
rows=[]; palettes={}
for mode,dark,hc,eink in [('dark',True,False,False),('light',False,False,False),('dark-hc',True,True,False),('light-hc',False,True,False),('eink',False,False,True)]:
    env=dict(dark=dark,hc=hc,eink=eink)
    # Placeholder references muted declared later.
    for name,expr in sorted(expressions,key=lambda x:x[0]=='placeholder'):env[name]=evaluate(expr,env)
    palettes[mode]={k:v for k,v in env.items() if isinstance(v,str)}
    for fg in ['text','muted','placeholder','accent','error','warning','success','border','divider','focus']:
        for bg in ['surface','card','hover','selected']:
            rows.append({'mode':mode,'foreground':fg,'background':bg,'fg_hex':env[fg],'bg_hex':env[bg],'ratio':round(ratio(env[fg],env[bg]),4),'threshold':3 if fg in ['border','divider','focus'] else 4.5,'usage':'candidate pair; confirm source usage'})
    for fg,back,op,usage in [('on-accent','accent',1,'primary icon button'),('text','card',.35,'disabled rule: whole card opacity; readable checkbox remains interactive'),('muted','card',.35,'disabled rule summary'),('text','card',.55,'category buttons remain clickable during query')]:
        background=env[back]
        if op == .35:
            background=blend(env[back],env['surface'],op)
        f=blend(env[fg],background,op)
        rows.append({'mode':mode,'foreground':fg,'background':back,'fg_hex':f,'bg_hex':background,'ratio':round(ratio(f,background),4),'threshold':4.5,'usage':usage})
with (a.out/'contrast.csv').open('w') as out:
    writer=csv.DictWriter(out,fieldnames=rows[0]); writer.writeheader();writer.writerows(rows)
(a.out/'palettes.json').write_text(json.dumps(palettes,indent=2)+'\n')
summary={'files':len(files),'lines':sum(f['lines'] for f in inventory),'instances':len(entries),'counts':{k:sum(f[k] for f in inventory) for k in ['touch','focus','accessible_roles','keyboard_handlers','live_regions','text_fields']},'contrast_pairs':len(rows),'below_threshold_candidates':sum(r['ratio']<r['threshold'] for r in rows)}
(a.out/'static-summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,ensure_ascii=False,indent=2))
