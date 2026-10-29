#!/usr/bin/env python3
"""Change output scale only inside our private virtual compositor, then run the probe."""
import json,os,subprocess,time
from pathlib import Path
if not os.environ.get('WAYLAND_DISPLAY','').startswith('lhc-a11y-'):
    raise SystemExit('Requires run-isolated.sh private compositor')
out=Path(os.environ['LHC_AUDIT_OUT'])
config=json.loads(subprocess.check_output(['kscreen-doctor','-j'],text=True))
name=next(o['name'] for o in config['outputs'] if o['connected'] and o['enabled'])
scale=os.environ['LHC_AUDIT_OUTPUT_SCALE']
subprocess.run(['kscreen-doctor',f'output.{name}.scale.{scale}'],check=True)
time.sleep(.3)
(out/'output.json').write_text(subprocess.check_output(['kscreen-doctor','-j'],text=True))
os.execvp('python3',['python3',os.environ['LHC_AUDIT_ROOT']+'/dev_docs/accessibility-audit-2026-10-29/tools/atspi_watch.py'])
