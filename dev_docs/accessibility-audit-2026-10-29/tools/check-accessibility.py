#!/usr/bin/env python3
"""Check a stable accessibility fixture through external AT-SPI in run-isolated.sh."""
import json
import os
from pathlib import Path
import subprocess
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi

out = Path(os.environ["LHC_AUDIT_OUT"])
root = Path(os.environ["LHC_AUDIT_ROOT"])
assert os.environ.get("LHC_DEV_DIR"), "run in the disposable audit session"
for name in ("ScreenReaderEnabled", "IsEnabled"):
    subprocess.run([
        "gdbus", "call", "--session", "--dest", "org.a11y.Bus", "--object-path",
        "/org/a11y/bus", "--method", "org.freedesktop.DBus.Properties.Set",
        "org.a11y.Status", name, "<true>",
    ], check=True, capture_output=True)


def walk(node):
    yield node
    for index in range(node.get_child_count()):
        child = node.get_child_at_index(index)
        if child:
            yield from walk(child)


with (out / "atspi-fixture-stderr.log").open("w") as log:
    proc = subprocess.Popen([str(root / "target/debug/examples/accessibility")],
                            env={**os.environ, "LHC_A11Y_INSPECT": "1"}, stderr=log)
    try:
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            windows = [n for n in walk(Atspi.get_desktop(0))
                       if n.get_name() == "LHC accessibility fixture"]
            if windows:
                nodes = list(walk(windows[0]))
                spins = [n for n in nodes if n.get_role_name() == "spin button"]
                if spins:
                    break
            time.sleep(0.1)
        else:
            raise AssertionError("fixture spinbox was not exported to AT-SPI")
        assert len(spins) == 1, len(spins)
        spin = spins[0]
        assert spin.get_name() == "Fixture number", spin.get_name()
        text = spin.get_text_iface()
        assert text is not None, "spinbox lost its native text interface"
        assert Atspi.Text.get_text(text, 0, -1) == "20"
        tab = next(n for n in nodes if n.get_name() == "Fixture tab" and n.get_role_name() == "page tab")
        assert tab.get_state_set().contains(Atspi.StateType.SELECTED)
        result = {
            "spinbox": {"name": spin.get_name(), "role": spin.get_role_name(),
                        "text": Atspi.Text.get_text(text, 0, -1), "characters": Atspi.Text.get_character_count(text)},
            "tab": {"name": tab.get_name(), "selected": True},
            "nodes": [{"role": n.get_role_name(), "name": n.get_name(),
                       "states": [s.value_nick for s in n.get_state_set().get_states()]}
                      for n in nodes],
        }
        (out / "atspi-fixture.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
        print("PASS external AT-SPI: one named spinbox with native text; selected tab")
    finally:
        proc.terminate()
        proc.wait(timeout=10)
