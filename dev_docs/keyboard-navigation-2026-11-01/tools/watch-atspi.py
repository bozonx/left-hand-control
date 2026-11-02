import json, os, subprocess, time
from pathlib import Path
import gi
gi.require_version('Atspi','2.0')
from gi.repository import Atspi, GLib
out=Path(os.environ['LHC_AUDIT_OUT'])
for name in ['ScreenReaderEnabled','IsEnabled']:
    subprocess.run(['gdbus','call','--session','--dest','org.a11y.Bus','--object-path','/org/a11y/bus','--method','org.freedesktop.DBus.Properties.Set','org.a11y.Status',name,'<true>'],check=True,capture_output=True)
Atspi.set_timeout(1000,1000)
events=[]
def observe(event,*args):
    try:
        if event.detail1: events.append({'name':event.source.get_name(),'role':event.source.get_role_name()})
    except Exception as e: events.append({'error':str(e)})
listener=Atspi.EventListener.new(observe)
listener.register('object:state-changed:focused')
log=(out/'keyboard-atspi-scenario.log').open('w')
proc=subprocess.Popen([os.environ['LHC_AUDIT_ROOT']+'/target/debug/examples/keyboard-navigation'],stdout=log,stderr=subprocess.STDOUT)
loop=GLib.MainLoop(); start=time.monotonic()
def poll():
    if proc.poll() is not None: loop.quit(); return False
    if time.monotonic()-start>90: proc.terminate(); loop.quit(); return False
    return True
GLib.timeout_add(50,poll); loop.run()
rc=proc.wait(); log.close()
(out/'keyboard-atspi-events.json').write_text(json.dumps(events,ensure_ascii=False,indent=2))
if rc: raise SystemExit(rc)
assert events, 'no external focused events'
assert any(e.get('role')=='page tab' for e in events), events
assert any(e.get('role') in ['push button','radio button','list item'] for e in events), events
print(f'PASS: {len(events)} external AT-SPI focused events')
