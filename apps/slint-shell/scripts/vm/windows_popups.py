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

    def profile_remap_restart():
        metadata = json.loads(probe.read(ready))
        environment = (f"$env:LHC_DEV_DIR = '{metadata['profile']}'; "
                       f"$env:LHC_PORT = '{metadata['port']}'; ")

        def command(*arguments):
            probe.ps(environment + f"& '{metadata['exe']}' " + " ".join(arguments) +
                     "; if ($LASTEXITCODE -ne 0) { throw 'Shell command failed' }", timeout=30)

        command("game", "off")
        command("toggle-mapper")
        probe.reset()
        vm.qmp.tap("a")
        probe.text("b")
        command("game", "on")
        probe.reset()
        vm.qmp.tap("a")
        probe.text("c")
        old_pid = metadata["pid"]
        probe.ps(f"Set-Content '{WORK}\\popups\\restart-request' -Value 1", timeout=30)

        def restarted():
            current = json.loads(probe.read(ready))
            return current if current["pid"] != old_pid else None

        metadata = wait_for("same-profile shell restart", 45, restarted)
        command("toggle-mapper")
        probe.reset()
        vm.qmp.tap("a")
        probe.text("c")
        command("game", "off")
        probe.reset()
        vm.qmp.tap("a")
        probe.text("b")
        command("toggle-mapper")
        probe.reset()
        vm.qmp.tap("a")
        probe.text("a")
        return "application profile -> mapper -> receiver: b; persisted game setting -> c across restart; off -> b; stopped -> a"

    app_crashed = False

    def crash_owner():
        nonlocal app_crashed
        metadata = json.loads(probe.read(ready))
        environment = (f"$env:LHC_DEV_DIR = '{metadata['profile']}'; "
                       f"$env:LHC_PORT = '{metadata['port']}'; ")
        ps(environment + f"& '{metadata['exe']}' toggle-mapper; "
           "if ($LASTEXITCODE -ne 0) { throw 'Mapper toggle failed' }", timeout=30)
        since = probe.reset()
        try:
            probe.press("f")
            wait_for("application holds virtual Ctrl", 10, lambda: (17, True) in probe.transitions(since))
            ps(f"$listeners = @(Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort {metadata['port']} -State Listen); "
               f"if ($listeners.Count -ne 1 -or $listeners[0].OwningProcess -ne {metadata['pid']}) "
               "{ throw 'Application ownership changed before crash' }; "
               f"Set-Content '{WORK}\\popups\\expect-crash' -Value 1; "
               f"Stop-Process -Id {metadata['pid']} -Force", timeout=30)
            app_crashed = True
            wait_for("application guard releases Ctrl before physical release", 10,
                     lambda: (17, False) in probe.transitions(since))
            probe.release("f")
            probe.receiver("focus")
            if probe.state()["modifiers_down"]:
                raise StepFailed(f"Application crash left modifiers held: {probe.state()}")
        finally:
            for key in list(probe.held):
                probe.release(key)
        probe.reset()
        vm.qmp.tap("a")
        probe.text("a")
        return "real application forced termination; guard releases Ctrl before physical release; native input works"

    completed = False
    try:
        report.step("shell-profile-remap-restart", profile_remap_restart, fatal=True)
        for name, popup, key, text in (
            ("popup-bmp-focus", "emoji", "q", "☕"),
            ("popup-surrogate-focus", "emoji", "w", "😀"),
            ("popup-zwj-focus", "emoji", "e", "👩‍💻"),
            ("popup-mixed-text-focus", "quick", "q", "Действие 02 😀"),
            ("popup-reopen-focus", "emoji", "q", "☕"),
        ):
            report.step(name, lambda p=popup, k=key, t=text: select(p, k, t), fatal=True)
            vm.qmp.screendump(vm.logs / f"{name}.png")
        report.step("shell-mapper-crash-held", crash_owner, fatal=True)
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
            if not app_crashed:
                metadata = json.loads(probe.read(ready))
                command = (f"$env:LHC_DEV_DIR = '{metadata['profile']}'; "
                           f"$env:LHC_PORT = '{metadata['port']}'; & '{metadata['exe']}' quit; "
                           "if ($LASTEXITCODE -ne 0) { throw 'Popup shell quit failed' }")
                probe.ps(command, timeout=30)
        except (StepFailed, subprocess.TimeoutExpired, ValueError, OSError):
            if completed:
                raise
            report.skip("popup-cleanup", "cleanup failed; original scenario failure preserved")
