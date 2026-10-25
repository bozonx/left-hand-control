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
from text_corpus import TEXT_CORPUS

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
        self.run_guest = ps
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

    def command(self, target, op, wait_response=True, **data):
        self.serial += 1
        request = {"id": self.serial, "op": op, **data}
        encoded = base64.b64encode(json.dumps(request).encode()).decode()
        name = "request" if target == "mapper" or target.startswith("mapper\\") else "command"
        path = f"{WORK}\\{target}\\{name}.json"
        self.ps(f"[IO.File]::WriteAllBytes('{path}.tmp', [Convert]::FromBase64String('{encoded}')); "
                f"if (Test-Path '{path}') {{ [IO.File]::Replace('{path}.tmp', '{path}', [NullString]::Value) }} "
                f"else {{ [IO.File]::Move('{path}.tmp', '{path}') }}", timeout=30)

        if not wait_response:
            return request["id"]

        def response():
            try:
                value = json.loads(self.read(f"{WORK}\\{target}\\response.json"))
                return value if value.get("id") == request["id"] else None
            except (StepFailed, ValueError):
                return None
        result = wait_for(f"{target} {op}", 15, response, interval=0.1)
        with (self.vm.logs / "mapper-mailbox.jsonl").open("a") as log:
            log.write(json.dumps({"target": target, "op": op, "response": result}, ensure_ascii=False) + "\n")
        return result

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
        self.vm.qmp.click(0.5, 0.5)
        self.receiver("focus")
        self.receiver("unpin")
        try:
            wait_for("receiver focus", 10, lambda: self.state()["form_active"] and self.state()["editor_focused"])
        except StepFailed:
            try:
                self.vm.qmp.screendump(self.vm.logs / "receiver-focus-failure.png")
            except (OSError, StepFailed):
                pass
            raise
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

    def crash_held(self):
        self.mapper("stop")
        result = self.command("mapper", "crash-start")
        if not result["ok"]:
            raise StepFailed(f"Could not start crash probe: {result}")
        wait_for("crash probe ready", 15, lambda: "True" in self.ps(f"Test-Path {WORK}\\mapper\\crash\\ready", timeout=30))
        config = {"rules": [{"key": key, "tapAction": modifier, "holdAction": modifier}
                            for key, modifier in (("KeyF", "ControlLeft"), ("KeyG", "ShiftRight"),
                                                  ("KeyH", "AltLeft"), ("KeyJ", "MetaRight"))]}
        result = self.command(r"mapper\crash", "start", config=config)
        if not result["ok"] or not result["status"]["running"]:
            raise StepFailed(f"Crash mapper did not start: {result}")
        self.receiver("trace")
        since = self.reset()
        for key in ("f", "g", "h", "j"):
            self.press(key)
            time.sleep(0.4)
        self.receiver("focus")
        if set(self.state()["modifiers_down"]) != {162, 161, 164, 92}:
            raise StepFailed(f"Expected held Ctrl/Shift/Alt/Win before crash: {self.state()}")
        result = self.command("mapper", "crash-kill")
        if not result["ok"]:
            raise StepFailed(f"Crash termination failed: {result}")
        wait_for("guard releases Ctrl/Shift/Alt/Win before physical release", 10,
                 lambda: {(16, False), (17, False), (18, False), (92, False)} <= set(self.transitions(since)))
        for key in ("f", "g", "h", "j"):
            self.release(key)
        self.receiver("focus")
        if self.state()["modifiers_down"]:
            raise StepFailed(f"Modifiers remain down after crash: {self.state()}")
        self.reset()
        self.vm.qmp.tap("a")
        self.text("a")
        self.mapper("start", config={"rules": [{"key": "KeyA", "tapAction": "KeyB"}]})
        self.receiver("trace")
        self.reset()
        self.vm.qmp.tap("a")
        self.text("b")
        return "forced owner termination releases Ctrl/Shift/Alt/Win before physical release; native input and mapper restart work"

    def guard_failure(self):
        self.config({"rules": [{"key": "KeyF", "tapAction": "ControlLeft", "holdAction": "ControlLeft"}]})
        since = self.reset()
        self.press("f")
        time.sleep(0.4)
        wait_for("virtual Ctrl held", 10, lambda: (17, True) in self.transitions(since))
        guard = self.mapper("status").get("guard_pid")
        if not guard:
            raise StepFailed("No input guard PID in diagnostic status")
        self.run_guest(f"Stop-Process -Id {int(guard)} -Force", timeout=30)
        wait_for("mapper stops after guard death", 10,
                 lambda: not self.command("mapper", "status")["status"]["running"])
        wait_for("fallback releases Ctrl before physical release", 10,
                 lambda: (17, False) in self.transitions(since))
        self.release("f")
        self.reset()
        self.vm.qmp.tap("a")
        self.text("a")
        self.mapper("start", config={})
        self.receiver("trace")
        return "guard death stops mapper, fallback releases Ctrl, native input and restart work"

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

    def secure_desktop_held(self):
        """Keys released on the Ctrl+Alt+Del screen never reach the hook.
        The layer held before it must not stay active afterwards."""
        self.config({"rules": [{"key": "KeyF", "tapAction": "KeyX", "layerId": "nav",
                                "holdTimeoutMs": 200}],
                     "layerKeymaps": {"nav": {"keys": {"KeyJ": "KeyK"}}}})
        self.reset()
        self.press("f")
        time.sleep(0.5)
        for key in ("ctrl", "alt", "delete"):
            self.press(key)
            time.sleep(0.05)
        time.sleep(1)
        for key in ("delete", "alt", "ctrl", "f"):
            self.release(key)
            time.sleep(0.05)
        time.sleep(3)
        self.vm.qmp.screendump(self.vm.logs / "secure-desktop.png")
        self.vm.qmp.tap("esc")
        time.sleep(3)
        self.reset()
        self.vm.qmp.tap("j", "a")
        self.text("ja")
        self.press("f")
        time.sleep(0.5)
        self.vm.qmp.tap("j")
        self.release("f")
        self.text("jak")
        return "layer and modifiers released on the secure desktop do not stay held"

    def unlock(self):
        def locked():
            return self.receiver("session-state")["locked"]
        if not locked():
            return
        password = self.ps(r"(Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon').DefaultPassword", timeout=30).rstrip("\r\n")
        for _ in range(8):
            if not locked():
                return
            self.vm.qmp.tap("spc")
            time.sleep(2)
            if not locked():
                return
            self.vm.qmp.tap("ctrl-a", "backspace")
            if password:
                self.vm.qmp.type_ascii(password)
            self.vm.qmp.tap("ret")
            time.sleep(2)
        wait_for("unlocked Windows desktop", 15, lambda: not locked())

    def lock_unlock(self):
        self.config({"rules": [{"key": "KeyA", "tapAction": "KeyB"}]})
        self.reset()
        self.receiver("lock")
        wait_for("Windows lock screen", 30, lambda: self.receiver("session-state")["locked"])
        self.vm.qmp.tap("a")
        self.unlock()
        if self.state()["text"]:
            raise StepFailed("Lock-screen input leaked into the background receiver")
        self.reset()
        self.vm.qmp.tap("a")
        self.text("b")
        return "real LockWorkStation, QMP credential input, mapper still remaps after unlock"

    def suspend_resume(self):
        self.config({"rules": [{"key": "KeyA", "tapAction": "KeyB"}]})
        before = self.ps("(Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToFileTimeUtc()", timeout=30).strip()
        capabilities = self.ps("powercfg /a", timeout=30)
        (self.vm.logs / "power-capabilities.txt").write_text(capabilities, encoding="utf-8")
        available = capabilities.split("The following sleep states are not available", 1)[0]
        if "Standby (S3)" not in available:
            raise StepFailed("Prepared Windows VM does not expose S3; see power-capabilities.txt")
        for _ in range(3):
            self.reset()
            self.command("receiver", "suspend", wait_response=False)
            wait_for("Windows S3", 60, lambda: self.vm.qmp.status() == "suspended")
            time.sleep(3)
            self.vm.qmp.execute("system_wakeup")
            self.vm.wait_ssh(timeout=120)
            after = self.ps("(Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToFileTimeUtc()", timeout=30).strip()
            if before != after:
                raise StepFailed("Windows rebooted instead of resuming")
            if self.receiver("session-state")["locked"]:
                self.unlock()
            if self.state()["text"]:
                raise StepFailed("Lock-screen input leaked into the background receiver")
            self.reset()
            self.vm.qmp.tap("a")
            self.text("b")
        return "3 QMP-confirmed S3/wakeup cycles; unchanged boot time; exact remap after each resume"

    def stress(self, seconds):
        deadline = time.monotonic() + seconds
        cycles, samples = 0, []
        while time.monotonic() < deadline or cycles < 3:
            self.config({"rules": [{"key": "KeyA", "tapAction": "KeyB"},
                                   {"key": "KeyY", "tapAction": "text:Привет 👩‍💻 é"}]})
            self.reset()
            self.vm.qmp.tap("a", "y")
            self.text("bПривет 👩‍💻 é")
            self.mapper("stop")
            self.mapper("start", config={})
            cycles += 1
            raw = self.ps(f"Get-Process -Id (Get-Content {WORK}\\mapper\\ready -Raw) | "
                          "Select-Object Id,PrivateMemorySize64,HandleCount | ConvertTo-Json -Compress", timeout=30)
            samples.append(json.loads(raw.lstrip("\ufeff")))
        warm = samples[1:]
        if len(warm) > 1:
            if warm[-1]["PrivateMemorySize64"] - warm[0]["PrivateMemorySize64"] > 64 * 1024 * 1024:
                raise StepFailed(f"Mapper memory growth exceeds 64 MiB: {samples}")
            if warm[-1]["HandleCount"] - warm[0]["HandleCount"] > 32:
                raise StepFailed(f"Mapper handle growth exceeds 32: {samples}")
        (self.vm.logs / "mapper-stress-resources.json").write_text(json.dumps(samples, indent=2))
        return f"{cycles} exact Unicode/remap/stop/start cycles over at least {seconds}s; memory/handles bounded"

    def process_tree_crash(self):
        self.mapper("stop")
        self.command("mapper", "crash-start")
        wait_for("crash probe ready", 15, lambda: "True" in self.ps(f"Test-Path {WORK}\\mapper\\crash\\ready", timeout=30))
        result = self.command(r"mapper\crash", "start", config={"rules": [
            {"key": "KeyF", "tapAction": "ControlLeft", "holdAction": "ControlLeft"}]})
        if not result["ok"]:
            raise StepFailed(f"Crash probe failed: {result}")
        since = self.reset()
        try:
            self.press("f")
            wait_for("crash probe holds Ctrl", 10, lambda: (17, True) in self.transitions(since))
            self.receiver("freeze-crash-tree", guard_pid=result["guard_pid"])
            before_kill = len(self.events())
            if (17, False) in self.transitions(since) or 162 not in self.state()["modifiers_down"]:
                raise StepFailed("Ctrl was released before owner-tree termination")
            self.run_guest(f"$owner = [int](Get-Content {WORK}\\mapper\\crash\\ready -Raw); "
                           "& taskkill /PID $owner /T /F; if ($LASTEXITCODE) { throw 'taskkill failed' }", timeout=30)
            wait_for("whole tree releases Ctrl", 10, lambda: (17, False) in self.transitions(before_kill))
            if self.state()["modifiers_down"]:
                raise StepFailed("Whole process-tree termination left a modifier held")
        finally:
            self.release("f")
            # Recovery follows the assertion; never count corrective key input as a pass.
            self.vm.qmp.tap("ctrl")
            self.mapper("start", config={})
        return "owner tree suspended then terminated with taskkill /T; independent guard releases Ctrl before corrective input"

    def unicode_corpus(self):
        try:
            for locale in ("en-US", "ru-RU"):
                self.receiver("locale", locale=locale)
                for sample in (TEXT_CORPUS, "строка 1\nстрока 2"):
                    self.config({"rules": [{"key": "KeyY", "tapAction": "text:" + sample}]})
                    self.reset()
                    self.vm.qmp.tap("y")
                    expected = sample.replace("\n", "\r\n")
                    self.text(expected)
                    if self.state()["code_points"] != list(map(ord, expected)):
                        raise StepFailed("Unicode corpus code points differ")
        finally:
            self.receiver("locale", locale="en-US")
        return "US/RU: punctuation, accents, skin tone, ZWJ, NFC/NFD and multiline (WinForms CRLF)"

    def notepad_receiver(self):
        corpus = "Привет, ёж 😀 👩‍💻 é"
        self.config({"rules": [{"key": "KeyY", "tapAction": "text:" + corpus}]})
        self.receiver("unpin")
        self.command("receiver", "notepad", wait_response=False)
        def state():
            try:
                return json.loads(self.read(f"{WORK}\\receiver\\notepad.json"))
            except (StepFailed, ValueError):
                return {}
        try:
            wait_for("owned interactive Notepad", 45, lambda: state().get("pid"))
            self.vm.qmp.click(0.5, 0.5)
            wait_for("Notepad foreground", 10, lambda: state().get("active"))
            for corpus in (corpus, "é" * 520 + " 👩‍💻 é"):
                self.config({"rules": [{"key": "KeyY", "tapAction": "text:" + corpus}]})
                self.vm.qmp.tap("ctrl-a")
                # Allow the real editor to finish selection before the mapped input.
                time.sleep(1)
                self.vm.qmp.tap("y")
                wait_for("exact Unicode in real Notepad", 60,
                         lambda: state().get("text", "").rstrip("\r\n") == corpus)
        except StepFailed:
            (self.vm.logs / "notepad-failure.json").write_text(json.dumps(
                {"expected": corpus, "actual": state(), "mapper": self.mapper("status")},
                ensure_ascii=False, indent=2))
            self.vm.qmp.screendump(self.vm.logs / "notepad-failure.png")
            raise
        finally:
            self.receiver("notepad-close")
            self.reset()
        return "QMP mapped input into real Notepad; exact Cyrillic/emoji/ZWJ/combining text and >15s paced text without expiry"

    def elevated_uipi(self):
        marker = "LHC-UIPI-MUST-NOT-ARRIVE"
        self.config({"rules": [{"key": "KeyA", "tapAction": "text:" + marker}]})
        self.reset()
        self.command("receiver", "elevate", wait_response=False)
        time.sleep(3)
        self.vm.qmp.screendump(self.vm.logs / "uac-prompt.png")
        self.vm.qmp.tap("alt-y")
        try:
            wait_for("elevated receiver", 45, lambda: "True" in self.ps(f"Test-Path '{WORK}\\elevated\\snapshot.json'", timeout=30))
            self.command("elevated", "show")
            self.vm.qmp.click(0.5, 0.5)
            self.command("elevated", "focus")
            self.command("elevated", "unpin")
            wait_for("elevated receiver owns foreground", 10,
                     lambda: json.loads(self.read(f"{WORK}\\elevated\\snapshot.json"))["form_active"])
            elevated = json.loads(self.read(f"{WORK}\\elevated\\snapshot.json"))
            if not elevated.get("elevated"):
                raise StepFailed("Receiver is not elevated; UIPI was not tested")
            self.vm.qmp.tap("a")
            time.sleep(1)
            actual = json.loads(self.read(f"{WORK}\\elevated\\snapshot.json"))["text"]
            if marker in actual or marker in self.state()["text"]:
                raise StepFailed("UIPI boundary bypassed or text reached the wrong receiver")
            events = self.mapper("status")["events"]
            if not actual and not any("TextInputFailed" in event or "ActionUnavailable" in event for event in events):
                raise StepFailed("Elevated input was swallowed without a failure notice")
            self.command("elevated", "blur")
            self.config({"rules": [{"key": "KeyA", "tapAction": "KeyB"}]})
            self.reset()
            self.vm.qmp.tap("a")
            self.text("b")
        finally:
            # The elevated fixture owns its UI; mailbox control hides it without
            # granting the controller a privileged process-termination API.
            try:
                self.command("elevated", "close")
            except (StepFailed, subprocess.TimeoutExpired):
                pass
        return "real UAC/elevated target; synthetic marker blocked; ordinary mapper input recovers"

    def run(self, start_at=None, extended=False, stress_seconds=180, only=None):
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
                ("mapper-secure-desktop-held", self.secure_desktop_held),
                ("mapper-crash-held", self.crash_held),
                ("mapper-guard-failure", self.guard_failure),
            )
            extended_scenarios = (
                ("mapper-lock-unlock", self.lock_unlock),
                ("mapper-suspend-resume", self.suspend_resume),
                ("mapper-stress", lambda: self.stress(stress_seconds)),
                ("mapper-unicode-corpus", self.unicode_corpus),
                ("mapper-notepad-unicode", self.notepad_receiver),
                ("mapper-elevated-uipi", self.elevated_uipi),
                ("mapper-process-tree-crash", self.process_tree_crash),
            )
            if only and only not in dict(scenarios + (extended_scenarios if extended else ())):
                raise StepFailed(f"Unknown mapper scenario: {only}")
            if start_at and start_at not in dict(scenarios + (extended_scenarios if extended else ())):
                raise StepFailed(f"Unknown mapper scenario: {start_at}")
            for name, scenario in scenarios:
                if only and name != only:
                    self.report.skip(name, f"diagnostic run selects {only}")
                    continue
                if start_at and name != start_at:
                    self.report.skip(name, f"diagnostic run starts at {start_at}")
                    continue
                start_at = None
                self.report.step(name, scenario, fatal=True)
                self.vm.qmp.screendump(self.vm.logs / f"{name}.png")
            if extended:
                for name, scenario in extended_scenarios:
                    if only and name != only:
                        self.report.skip(name, f"diagnostic run selects {only}")
                        continue
                    if start_at and name != start_at:
                        self.report.skip(name, f"diagnostic run starts at {start_at}")
                        continue
                    start_at = None
                    self.report.step(name, scenario)
                    self.vm.qmp.screendump(self.vm.logs / f"{name}.png")
            else:
                for name, _ in extended_scenarios:
                    self.report.skip(name, "run with --extended")
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
