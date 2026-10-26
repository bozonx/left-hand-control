#!/usr/bin/env python3
import csv
import math
import statistics
import sys
from collections import defaultdict

groups = defaultdict(lambda: defaultdict(list))
trials = defaultdict(set)
with open(sys.argv[1], newline="") as source:
    for row in csv.DictReader(source):
        if row["source"] == "startup":
            continue
        key = row["window"], row["source"]
        trials[key].add(row["trial"])
        groups[key][row["event"]].append(float(row["elapsed_ms"]))
for key, events in groups.items():
    print("/".join(key), "trials:", len(trials[key]))
    for event, values in events.items():
        values.sort()
        print(f"  {event}: n={len(values)} p50={statistics.median(values):.3f} "
              f"p95={values[math.ceil(len(values) * .95) - 1]:.3f} max={max(values):.3f} ms")
    print(f"  focus: {len(events.get('t4_focused', []))}/{len(trials[key])}")
