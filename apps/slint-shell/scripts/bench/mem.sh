#!/usr/bin/env bash
set -euo pipefail
python3 - "$@" <<'PY'
import csv
import pathlib
import sys

root = int(sys.argv[1])
label = sys.argv[2] if len(sys.argv) > 2 else 'sample'
parents = {}
for path in pathlib.Path('/proc').glob('[0-9]*/status'):
    try:
        fields = dict(line.split(':', 1) for line in path.read_text().splitlines())
        parents.setdefault(int(fields['PPid']), []).append(int(path.parent.name))
    except (FileNotFoundError, ProcessLookupError):
        continue
pids = [root]
for pid in pids:
    pids.extend(parents.get(pid, []))
rss = pss = 0
for pid in pids:
    fields = dict(line.split(':', 1) for line in pathlib.Path(f'/proc/{pid}/smaps_rollup').read_text().splitlines()[1:])
    rss += int(fields['Rss'].split()[0])
    pss += int(fields['Pss'].split()[0])
writer = csv.writer(sys.stdout)
writer.writerow(['label', 'processes', 'rss_kib', 'pss_kib'])
writer.writerow([label, len(pids), rss, pss])
PY
