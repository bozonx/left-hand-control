"""Hardware-input Windows mapper acceptance, driven by the host over QMP.
The Rust stand controls the public core runtime; the WinForms receiver records
what a normal target application actually receives. Neither uses SendKeys.
"""

import base64
import json
import subprocess
import time
from datetime import datetime

from vmlib import KEYBOARD_PORT, StepFailed, wait_for

WORK = r"D:\lhc-e2e\out"


def retry_guest_io(operation):
    """Retry idempotent mailbox I/O, never keyboard input or process launches."""
    for attempt in range(3):
        try:
            return operation()
        except (StepFailed, subprocess.TimeoutExpired) as error:
            if isinstance(error, StepFailed) and "exited with 255" not in str(error):
                raise
            if attempt == 2:
                raise
            time.sleep(0.5)


class WindowsMapper:
    def __init__(self, vm, ps, report):
        self.vm, self.report = vm, report
        self.ps = lambda command, **kwargs: retry_guest_io(lambda: ps(command, **kwargs))
        self.serial = 0
        self.held = set()

    def read(self, path):
        raw = self.ps(
            f"$s = [IO.File]::Open('{path}', 'Open', 'Read', "
            "([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete)); "
            "$r = [IO.StreamReader]::new($s, [Text.Encoding]::UTF8); "
            "try { $r.ReadToEnd() } finally { $r.Dispose() }", timeout=30)
        return raw.lstrip("\ufeff")

    def command(self, target, op, **data):
        self.serial += 1
        request = {"id": self.serial, "op": op, **data}
        encoded = base64.b64encode(json.dumps(request).encode()).decode()
        name = "request" if target == "mapper" else "command"
        path = f"{WORK}\\{target}\\{name}.json"
        self.ps(f"[IO.File]::WriteAllBytes('{path}.tmp', [Convert]::FromBase64String('{encoded}')); "
                f"if (Test-Path '{path}') {{ [IO.File]::Replace('{path}.tmp', '{path}', [NullString]::Value) }} "
                f"else {{ [IO.File]::Move('{path}.tmp', '{path}') }}", timeout=30)

        def response():
            try:
                value = json.loads(self.read(f"{WORK}\\{target}\\response.json"))
                return value if value.get("id") == request["id"] else None
            except (StepFailed, ValueError):
                return None
        return wait_for(f"{target} {op}", 15, response, interval=0.1)

    def mapper(self, op, **data):
        result = self.command("mapper", op, **data)
        if not result["ok"]:
            raise StepFailed(f"mapper {op}: {result}")
        if op not in ("stop", "shutdown") and not result["status"]["running"]:
            raise StepFailed(f"mapper stopped: {result}")
        return result

    def receiver(self, op, **data):
        result = self.command("receiver", op, **data)
        if not result["ok"]:
            raise StepFailed(f"receiver {op}: {result}")
        return result

    def state(self):
        return json.loads(self.read(f"{WORK}\\receiver\\snapshot.json"))

    def events(self):
        lines = self.read(f"{WORK}\\receiver\\events.jsonl").splitlines()
        # An event may still be appending. Keep complete records only.
        records = []
        for line in lines:
            try:
                records.append(json.loads(line))
            except ValueError:
                break
        return records

    def reset(self):
        self.receiver("reset")
        self.receiver("show")
        wait_for("receiver focus", 10, lambda: self.state()["form_active"] and self.state()["editor_focused"])
        return len(self.events())

    def text(self, expected):
        wait_for(f"text {expected!r}", 10, lambda: self.state()["text"] == expected)
        time.sleep(0.3)
        actual = self.state()["text"]
        if actual != expected:
            raise StepFailed(f"Late/duplicate input: {actual!r}, expected {expected!r}")

    def transitions(self, since):
        return [(e["key_code"], e["event"] == "key-down") for e in self.events()[since:]
                if e["event"] in ("key-down", "key-up")]

    def exact_keys(self, since, expected):
        actual = self.transitions(since)
        if actual != expected:
            raise StepFailed(f"Unexpected transitions: {actual}; expected {expected}")

    def press(self, key):
        self.held.add(key)
        self.vm.qmp.press(key)

    def release(self, key):
        self.vm.qmp.release(key)
        self.held.discard(key)

    def config(self, config):
        self.mapper("config", config=config)

    def passthrough(self):
        since = self.reset()
        self.vm.qmp.tap("a", "b", "c")
        self.text("abc")
        self.exact_keys(since, [(key, down) for key in (65, 66, 67) for down in (True, False)])

    def remap(self):
        self.config({"rules": [{"key": "KeyA", "tapAction": "KeyB"},
                               {"key": "KeyB", "tapAction": "KeyC"},
                               {"key": "KeyS", "tapAction": None, "holdAction": None}]})
        since = self.reset()
        self.vm.qmp.tap("a", "s", "d")
        self.text("bd")
        self.exact_keys(since, [(key, down) for key in (66, 68) for down in (True, False)])
        return "A->B, B->C: injected B is not remapped; S swallowed; D passes through"

    def modifiers(self):
        self.config({})
        since = self.reset()
        self.vm.qmp.tap("shift-x", "shift_r-x", "ctrl-home", "shift-end", "o", "k")
        self.text("ok")
        transitions = self.transitions(since)
        for key in (16, 17, 35, 36, 88):
            if (key, True) not in transitions or (key, False) not in transitions:
                raise StepFailed(f"Missing modifier/extended transition for {key}: {transitions}")
        self.config({"rules": [{"key": key, "tapAction": action} for key, action in (
            ("ShiftLeft", "KeyL"), ("ShiftRight", "KeyR"),
            ("ControlLeft", "KeyC"), ("ControlRight", "KeyD"),
            ("AltLeft", "KeyE"), ("AltRight", "KeyF"),
        )]})
        self.reset()
        self.vm.qmp.tap("shift", "shift_r", "ctrl", "ctrl_r", "alt", "alt_r")
        self.text("lrcdef")
        return "left/right Shift, Ctrl and Alt distinguished; Shift+X, Ctrl+Home, Shift+End"

    def repeat(self):
        self.config({"rules": [{"key": "KeyA", "tapAction": "KeyB", "holdAction": "KeyB"}]})
        since = self.reset()
        self.press("a")
        time.sleep(1.5)
        self.release("a")
        wait_for("repeated mapped key", 10, lambda: len(self.state()["text"]) >= 2)
        time.sleep(0.3)
        text = self.state()["text"]
        keys = self.transitions(since)
        if set(text) != {"b"} or sum(k == (66, True) for k in keys) < 2 or keys[-1] != (66, False):
            raise StepFailed(f"Mapped repeat/release failed: {text!r}, {keys}")
        self.vm.qmp.tap("d")
        self.text(text + "d")

    def tap_hold_layer(self):
        self.config({"rules": [{"key": "KeyF", "tapAction": "KeyX", "holdAction": "ShiftLeft"}]})
        self.reset()
        self.vm.qmp.tap("f")
        self.press("f")
        time.sleep(0.4)
        self.vm.qmp.tap("a")
        self.release("f")
        self.text("xA")
        self.config({"rules": [{"key": "Tab", "layerId": "nav", "tapAction": None, "holdAction": None}],
                     "layerKeymaps": {"nav": {"keys": {"KeyJ": "KeyX"}}},
                     "settings": {"tapDecision": "holdOnOtherKeyPress"}})
        self.reset()
        self.press("tab")
        self.vm.qmp.tap("j")
        self.release("tab")
        self.vm.qmp.tap("j")
        self.text("xj")

    def chord_and_double_tap(self):
        self.config({"rules": [{"key": "KeyQ+KeyW", "tapAction": "Shift+KeyX"},
                               {"key": "KeyD", "doubleTapAction": "KeyZ", "doubleTapTimeoutMs": 300}]})
        self.reset()
        self.vm.qmp.tap("q-w", "d", "d", delay=0.03)
        self.text("Xz")

    def live_config_held(self):
        self.config({"rules": [{"key": "KeyF", "tapAction": None, "holdAction": "ShiftLeft"}]})
        since = self.reset()
        self.press("f")
        wait_for("virtual Shift down", 10, lambda: (16, True) in self.transitions(since))
        self.config({"rules": [{"key": "KeyF", "tapAction": "KeyZ", "holdAction": "ControlLeft"}]})
        self.vm.qmp.tap("a")
        self.release("f")
        self.vm.qmp.tap("f")
        self.text("Az")
        keys = self.transitions(since)
        if (16, False) not in keys or any(key == 17 for key, _ in keys):
            raise StepFailed(f"Config changed an already held modifier: {keys}")

    def macro(self):
        config = {"rules": [{"key": "KeyM", "tapAction": "macro:hi"}],
                  "macros": [{"id": "hi", "steps": [{"action": "KeyH"}, {"action": "pause:500"}, {"action": "KeyI"}]}]}
        self.config(config)
        since = self.reset()
        self.vm.qmp.tap("m")
        self.text("hi")
        downs = [e for e in self.events()[since:] if e["event"] == "key-down"]
        if [e["key_code"] for e in downs] != [72, 73]:
            raise StepFailed(f"Macro order: {downs}")
        pause = (datetime.fromisoformat(downs[1]["timestamp"]) - datetime.fromisoformat(downs[0]["timestamp"])).total_seconds()
        if pause < 0.45:
            raise StepFailed(f"Macro pause too short: {pause}")
        config["macros"][0]["steps"][1]["action"] = "pause:8000"
        self.config(config)
        self.reset()
        self.vm.qmp.tap("m")
        wait_for("first macro step", 10, lambda: self.state()["text"] == "h")
        self.config({"rules": [{"key": "KeyA", "tapAction": "KeyZ"}]})
        time.sleep(8.2)
        self.text("h")
        self.vm.qmp.tap("a")
        self.text("hz")
        return "ordered steps and >=500ms pause; live config cancels remaining macro"

    def unicode_layout(self):
        self.config({"rules": [{"key": "KeyA", "tapAction": "text:Привет 😀"}]})
        self.reset()
        self.vm.qmp.tap("a")
        self.text("Привет 😀")
        if self.state()["code_points"] != list(map(ord, "Привет 😀")):
            raise StepFailed("Unicode code points differ")
        self.config({})
        self.receiver("locale", locale="ru-RU")
        self.reset()
        self.vm.qmp.tap("f", "d")
        self.text("ав")
        self.receiver("locale", locale="en-US")
        self.reset()
        self.vm.qmp.tap("f", "d")
        self.text("fd")

    def focus_cancel(self):
        self.config({"rules": [{"key": "KeyA", "tapAction": "text:" + "x" * 4096}]})
        self.reset()
        # The receiver changes foreground after its first character, while
        # the real mapper text worker is still sending the remaining text.
        self.receiver("hide-on-input")
        self.vm.qmp.tap("a")
        wait_for("receiver hides during text", 10, lambda: not self.state()["form_active"])

        def cancelled():
            result = self.mapper("status")
            return any("TextInputFailed" in event and "cancel" in event for event in result["events"])
        wait_for("text worker reports focus cancellation", 10, cancelled)
        self.receiver("show")
        text = self.state()["text"]
        if not text or len(text) >= 4096:
            raise StepFailed(f"Focus cancellation did not interrupt text: {len(text)} characters")
        time.sleep(0.5)
        if self.state()["text"] != text:
            raise StepFailed("Cancelled text resumed after returning focus")
        self.config({})
        self.reset()
        self.vm.qmp.tap("a")
        self.text("a")
        return "receiver changes foreground mid-text; worker reports cancellation and does not resume"

    def stop_held(self):
        self.config({"rules": [{"key": "KeyF", "tapAction": None, "holdAction": "ControlLeft"}]})
        since = self.reset()
        self.press("f")
        wait_for("virtual Ctrl down", 10, lambda: (17, True) in self.transitions(since))
        self.mapper("stop")
        wait_for("shutdown releases Ctrl", 10, lambda: (17, False) in self.transitions(since))
        self.release("f")
        # Once hooks stop, the still-held physical F legitimately repeats natively.
        # Clear that text after releasing it; reset does not change keyboard state.
        self.reset()
        self.vm.qmp.tap("a", "b")
        self.text("ab")
        self.mapper("start", config={})
        self.receiver("trace")
        self.reset()
        self.vm.qmp.tap("a")
        self.text("a")
        return "stop releases virtual Ctrl before physical release; native input and restart work"

    def lock_and_unavailable(self):
        self.mapper("lock-probe")
        self.reset()
        result = self.mapper("execute", action="sys:switchDesktop1")
        if not any("ActionUnavailable" in event for event in result["events"]):
            raise StepFailed(f"No unsupported-action notice: {result}")
        self.text("")
        return "second process rejected by session mutex; unsupported system action reports notice"

    def replug(self):
        self.config({"rules": [{"key": "KeyA", "tapAction": "KeyB"}]})

        def keyboards():
            raw = self.ps("ConvertTo-Json -Compress -InputObject @(Get-PnpDevice -Class Keyboard -PresentOnly | "
                          "Where-Object Status -eq 'OK' | Select-Object -ExpandProperty InstanceId)", timeout=30)
            return set(json.loads(raw.lstrip("\ufeff")))

        before = keyboards()
        if not before:
            raise StepFailed("No ready keyboard before USB replug")
        self.vm.qmp.execute("device_del", id="kbd0")
        wait_for("Windows removes the USB keyboard", 30, lambda: before - keyboards())
        self.vm.qmp.execute("device_add", driver="usb-kbd", id="kbd0", port=KEYBOARD_PORT)
        wait_for("Windows USB keyboard driver ready", 30, lambda: before <= keyboards())
        self.reset()
        self.vm.qmp.tap("a")
        self.text("b")
        self.mapper("status")

    def run(self, start_at=None):
        wait_for("mapper stand", 30, lambda: "True" in self.ps(f"Test-Path {WORK}\\mapper\\ready", timeout=30))
        completed = False
        try:
            self.mapper("start", config={})
            self.receiver("trace")
            scenarios = (
                ("mapper-passthrough", self.passthrough),
                ("mapper-remap-swallow-recursion", self.remap),
                ("mapper-modifiers-extended", self.modifiers),
                ("mapper-repeat", self.repeat),
                ("mapper-tap-hold-layer", self.tap_hold_layer),
                ("mapper-chord-double-tap", self.chord_and_double_tap),
                ("mapper-live-config-held", self.live_config_held),
                ("mapper-macro-live-config-cancel", self.macro),
                ("mapper-unicode-us-ru", self.unicode_layout),
                ("mapper-focus-cancel", self.focus_cancel),
                ("mapper-stop-held-restart", self.stop_held),
                ("mapper-replug", self.replug),
                ("mapper-lock-unavailable", self.lock_and_unavailable),
            )
            if start_at and start_at not in dict(scenarios):
                raise StepFailed(f"Unknown mapper scenario: {start_at}")
            for name, scenario in scenarios:
                if start_at and name != start_at:
                    self.report.skip(name, f"diagnostic run starts at {start_at}")
                    continue
                start_at = None
                self.report.step(name, scenario, fatal=True)
                self.vm.qmp.screendump(self.vm.logs / f"{name}.png")
            self.report.skip("mapper-crash-held", "not accepted: forced process termination cannot run virtual-key cleanup")
            self.report.skip("mapper-secure-session", "elevated target, secure desktop and lock/unlock require separate acceptance")
            self.report.skip("mapper-suspend", "guest suspend/resume requires separate acceptance")
            self.report.skip("mapper-physical-hardware", "VM input does not accept physical Windows hardware")
            completed = True
        finally:
            try:
                try:
                    result = self.command("mapper", "stop")
                    if not result["ok"] and result["status"]["running"]:
                        raise StepFailed(f"Mapper cleanup failed: {result}")
                except (StepFailed, subprocess.TimeoutExpired):
                    if completed:
                        raise
                    self.report.skip("mapper-cleanup", "cleanup failed; original scenario failure preserved")
            finally:
                for key in list(self.held):
                    self.release(key)
