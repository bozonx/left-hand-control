#!/usr/bin/env python3
"""Collect the external AT-SPI tree of the disposable probe in a private session."""
import json, os, subprocess, time
from pathlib import Path
import gi
gi.require_version('Atspi','2.0')
from gi.repository import Atspi
out=Path(os.environ['LHC_AUDIT_OUT'])
# Activate a11y on this private D-Bus only.
r=subprocess.run(['gdbus','call','--session','--dest','org.a11y.Bus','--object-path','/org/a11y/bus','--method','org.freedesktop.DBus.Properties.Set','org.a11y.Status','ScreenReaderEnabled','<true>'],text=True,capture_output=True)
(out/'atspi-activation.txt').write_text(r.stdout+r.stderr)
r2=subprocess.run(['gdbus','call','--session','--dest','org.a11y.Bus','--object-path','/org/a11y/bus','--method','org.freedesktop.DBus.Properties.Set','org.a11y.Status','IsEnabled','<true>'],text=True,capture_output=True)
(out/'atspi-activation.txt').write_text(r.stdout+r.stderr+r2.stdout+r2.stderr)
Atspi.set_timeout(1000,1000)
proc=subprocess.Popen([os.environ['LHC_AUDIT_ROOT']+'/target/debug/examples/a11y-probe'],stdout=subprocess.PIPE,stderr=open(out/'probe-stderr.log','w'),text=True)
def walk(n,depth=0):
    if depth>40:return {'truncated':True}
    try:
        state=n.get_state_set()
        x={'role':n.get_role_name(),'name':n.get_name(),'description':n.get_description(),'states':[str(s.value_nick) for s in state.get_states()],'children':[]}
        for i in range(min(n.get_child_count(),1000)):
            c=n.get_child_at_index(i)
            if c:x['children'].append(walk(c,depth+1))
        return x
    except Exception as e:return {'error':str(e)}
for line in proc.stdout:
    print(line,flush=True,end='')
    if line.startswith('DUMP '):
        name=line.strip().split(' ',1)[1]
        tree=walk(Atspi.get_desktop(0))
        (out/(name+'-atspi.json')).write_text(json.dumps(tree,ensure_ascii=False,indent=2)+'\n')
rc=proc.wait();raise SystemExit(rc)
