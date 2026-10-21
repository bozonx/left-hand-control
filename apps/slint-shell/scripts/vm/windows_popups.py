"""Native hotkey -> popup selection -> foreground editor acceptance."""

import json
import subprocess

from vmlib import StepFailed, wait_for
from windows_mapper import WindowsMapper, WORK


def check_popups(vm, ps, report):
    probe = WindowsMapper(vm, ps, report)
    ready = f"{WORK}\\popups\\ready.json"
    wait_for("popup shell", 45, lambda: "True" in probe.ps(f"Test-Path '{ready}'", timeout=30))

    def metrics():
        return probe.read(f"{WORK}\\popups\\metrics.csv").splitlines()

    def count(stage):
        return sum(f",{stage}," in line for line in metrics())

    def select(popup, key, expected):
        probe.reset()
        before = count("t2_shown")
        vm.qmp.tap("ctrl-alt-f11" if popup == "emoji" else "ctrl-alt-f12")
        wait_for("popup shown", 10, lambda: count("t2_shown") > before)
        wait_for("popup owns focus", 10, lambda: not probe.state()["form_active"])
        vm.qmp.screendump(vm.logs / f"popup-{popup}-{key}-shown.png")
        vm.qmp.tap(key)
        vm.qmp.screendump(vm.logs / f"popup-{popup}-{key}-selected.png")
        probe.text(expected)
        state = probe.state()
        if not state["form_active"] or not state["editor_focused"]:
            raise StepFailed(f"Text arrived without restoring editor focus: {state}")
        if state["code_points"] != [ord(character) for character in expected]:
            raise StepFailed(f"Unexpected Unicode code points: {state}")
        vm.qmp.tap("a")
        probe.text(expected + "a")
        return "physical hotkey and cell key; exact Unicode once; focus restored; modifiers released"

    completed = False
    try:
        for name, popup, key, text in (
            ("popup-bmp-focus", "emoji", "q", "☕"),
            ("popup-surrogate-focus", "emoji", "w", "😀"),
            ("popup-zwj-focus", "emoji", "e", "👩‍💻"),
            ("popup-mixed-text-focus", "quick", "q", "Действие 02 😀"),
            ("popup-reopen-focus", "emoji", "q", "☕"),
        ):
            report.step(name, lambda p=popup, k=key, t=text: select(p, k, t), fatal=True)
            vm.qmp.screendump(vm.logs / f"{name}.png")
        completed = True
    except BaseException:
        try:
            vm.qmp.screendump(vm.logs / "popup-failure.png")
        except (StepFailed, OSError):
            report.skip("popup-failure-screenshot", "screenshot failed; original scenario failure preserved")
        raise
    finally:
        # Only this isolated instance owns the advertised profile and port.
        try:
            metadata = json.loads(probe.read(ready))
            command = (f"$env:LHC_DEV_DIR = '{metadata['profile']}'; "
                       f"$env:LHC_PORT = '{metadata['port']}'; & '{metadata['exe']}' quit; "
                       "if ($LASTEXITCODE -ne 0) { throw 'Popup shell quit failed' }")
            probe.ps(command, timeout=30)
        except (StepFailed, subprocess.TimeoutExpired, ValueError, OSError):
            if completed:
                raise
            report.skip("popup-cleanup", "cleanup failed; original scenario failure preserved")
