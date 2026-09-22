# Windows testing in KVM/QEMU

This document describes the reproducible Windows 11 test environment for Left Hand Control and the Slint prototype. Use a full VM with an interactive desktop. Docker and Windows containers cannot validate native windows, tray behavior, foreground activation, global hotkeys, `SendInput`, WebView2, DPI, or keyboard-layout integration.

KVM/QEMU is the primary local hypervisor on Linux. It gives near-native CPU execution, scriptable disks and snapshots, UEFI Secure Boot, TPM 2.0, and a QEMU monitor that can inject emulated hardware keys. VirtualBox is acceptable only as a fallback when KVM is unavailable; do not mix performance results from the two hypervisors.

## Host requirements

Install QEMU, KVM, OVMF, `swtpm`, and `socat`. Package names vary by distribution. The host user must be allowed to use `/dev/kvm`.

Check acceleration before installing Windows:

```bash
test -r /dev/kvm && test -w /dev/kvm
qemu-system-x86_64 -accel help
```

Use an official Windows 11 Enterprise Evaluation x64 ISO. Record its filename and SHA-256 rather than relying on the download URL, which Microsoft changes. The first test VM used:

```text
windows-11-enterprise-eval-25h2-en-us.iso
SHA-256 a61adeab895ef5a4db436e0a7011c92a2ff17bb0357f58b13bbc4062e535e7b9
```

The checksum identifies that test image; it is not a permanent checksum for future Windows releases.

## VM storage and firmware

Keep VM artifacts outside the repository:

```bash
vm_dir=/mnt/disk2/vm/left-hand-control-windows
mkdir -p "$vm_dir/tpm"
qemu-img create -f qcow2 "$vm_dir/windows11-lhc.qcow2" 64G
cp /usr/share/edk2/x64/OVMF_VARS.secboot.4m.fd "$vm_dir/OVMF_VARS.secboot.4m.fd"
```

Paths to OVMF files differ between distributions. Use the Secure Boot code image and a writable copy of the matching variables image. A plain BIOS VM or OVMF without Secure Boot reaches the Windows installer but Windows 11 setup rejects it with “The PC must support Secure Boot.” Do not bypass this requirement for the canonical test VM.

Start the software TPM in a separate terminal:

```bash
vm_dir=/mnt/disk2/vm/left-hand-control-windows
rm -f "$vm_dir/tpm/swtpm.sock"
swtpm socket \
  --tpm2 \
  --tpmstate dir="$vm_dir/tpm" \
  --ctrl type=unixio,path="$vm_dir/tpm/swtpm.sock" \
  --log file="$vm_dir/swtpm.log",level=20
```

Do not delete the TPM directory after Windows has been installed. Windows may bind state to that TPM.

## Canonical QEMU launch

Use 4 vCPU, 6 GiB RAM, an emulated tablet, and QEMU user networking. The e1000e adapter works with the Windows installer without a separate virtio driver ISO. Keep a monitor socket: it is the reliable way to test F13 or Scroll Lock when the physical keyboard lacks those keys.

```bash
vm_dir=/mnt/disk2/vm/left-hand-control-windows
qemu-system-x86_64 \
  -name 'LHC Windows 11' \
  -enable-kvm \
  -machine q35,accel=kvm,smm=on \
  -global driver=cfi.pflash01,property=secure,value=on \
  -cpu host,hv_relaxed,hv_vapic,hv_spinlocks=0x1fff,hv_time \
  -smp 4 \
  -m 6144 \
  -drive if=pflash,format=raw,readonly=on,file=/usr/share/edk2/x64/OVMF_CODE.secboot.4m.fd \
  -drive if=pflash,format=raw,file="$vm_dir/OVMF_VARS.secboot.4m.fd" \
  -chardev socket,id=chrtpm,path="$vm_dir/tpm/swtpm.sock" \
  -tpmdev emulator,id=tpm0,chardev=chrtpm \
  -device tpm-tis,tpmdev=tpm0 \
  -device ich9-ahci,id=ahci \
  -drive file="$vm_dir/windows11-lhc.qcow2",if=none,id=osdisk,format=qcow2,discard=unmap \
  -device ide-hd,drive=osdisk,bus=ahci.0,bootindex=1 \
  -drive file="$vm_dir/windows-11-enterprise-eval-25h2-en-us.iso",if=none,id=install,format=raw,readonly=on \
  -device ide-cd,drive=install,bus=ahci.1,bootindex=2 \
  -device qemu-xhci \
  -device usb-tablet \
  -device usb-kbd \
  -device e1000e,netdev=net0 \
  -netdev user,id=net0 \
  -device virtio-rng-pci \
  -monitor unix:"$vm_dir/qemu-monitor.sock",server=on,wait=off \
  -display gtk,gl=off \
  -serial none
```

After installation, remove the ISO drive and its CD device from the launch command. Leaving it attached can return the VM to setup after a boot-order change.

The GTK display used above does not provide a host/guest clipboard. This is expected, not an application defect. SPICE plus Windows guest tools can be configured later, but the canonical setup must remain usable without shared clipboard integration.

## Windows installation

Install Windows into the qcow2 disk and create an unprivileged local test user named `user`. Keep English (US) and Russian input languages installed.

Enterprise Evaluation can enter a work-or-school OOBE flow. Prefer the installer’s local account or domain-join option. If the screen only offers organization sign-in, disconnect networking during OOBE and use the Windows local-account path available in that image. Reconnect after the local account reaches the desktop. Do not enter a real organization account into the disposable test VM.

Once the desktop works:

1. Shut Windows down from the Start menu.
2. Wait for QEMU to exit completely.
3. Create a clean snapshot while no process has the qcow2 file open.

```bash
vm_dir=/mnt/disk2/vm/left-hand-control-windows
qemu-img snapshot -c windows-clean "$vm_dir/windows11-lhc.qcow2"
qemu-img snapshot -l "$vm_dir/windows11-lhc.qcow2"
```

`qemu-img snapshot` fails with a write-lock error while QEMU is running. Never force it against an active disk. With the monitor socket, an online snapshot can instead be requested deliberately with QEMU monitor commands, but offline snapshots are simpler for the baseline.

## Moving files into the guest

With QEMU user networking, Windows reaches the host as `10.0.2.2`. Serve a prepared archive or small bootstrap script from the host:

```bash
python3 -m http.server 18080 \
  --bind 0.0.0.0 \
  --directory /mnt/disk2/vm/left-hand-control-windows
```

Download it in Windows PowerShell:

```powershell
Invoke-WebRequest http://10.0.2.2:18080/bootstrap.ps1 -OutFile $env:TEMP\bootstrap.ps1
& $env:TEMP\bootstrap.ps1
```

Short `irm ... | iex` endpoints are useful during an interactive debugging session when clipboard sharing is unavailable, but committed documentation and repeatable automation should use a downloaded, inspectable `.ps1` file. Bind the host server only for the duration of the test and do not serve secrets or the entire home directory.

For a source snapshot, archive the exact commit plus intentional uncommitted patches, record both, and extract it to `C:\lhc`. A stale archive was one source of confusing Windows results during the Slint pilot; updating only a helper script does not update the source already extracted in the guest.

## Toolchain installation

Open PowerShell as the test user. Install the MSVC build tools and Rust from the `winget` community source explicitly:

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools --exact --source winget `
  --accept-package-agreements --accept-source-agreements `
  --override '--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'

winget install --id Rustlang.Rustup --exact --source winget `
  --accept-package-agreements --accept-source-agreements
```

Specifying `--source winget` matters. On the first VM, the Microsoft Store source had a certificate mismatch and `winget` printed `0x8a15005e`; the matching packages were still visible in the working `winget` source. A bootstrap that continues after this failure can misleadingly print “complete” even though nothing was installed. Check `$LASTEXITCODE` after every native installer.

Close PowerShell and open a new one so `%USERPROFILE%\.cargo\bin` is present, then run:

```powershell
rustup default stable
rustc --version
cargo --version
```

For the full Tauri application also install Node 20, pnpm 9, WebView2 Runtime, and `tauri-driver` as described in [E2E Desktop Testing](e2e-linux-kde.md#windows-vm).

## Build workflow

Use debug builds while iterating on Windows-only code:

```powershell
Set-Location C:\lhc
cargo test --locked --manifest-path prototypes/slint-shell/Cargo.toml --bin slint-shell
cargo build --locked --manifest-path prototypes/slint-shell/Cargo.toml --bin slint-shell
```

Run the release build only for the final validation:

```powershell
cargo build --release --locked --manifest-path prototypes/slint-shell/Cargo.toml --bin slint-shell
```

The initial Slint release build took about eleven minutes in the 4-vCPU/6-GiB VM because release LTO and host swapping dominated the link. A quiet linker is not necessarily hung. Check Task Manager or the QEMU process before interrupting it. Do not compare build time or runtime performance from a swapping VM with a physical Linux result.

PowerShell displays output written to stderr in red and may wrap a successful native command as `NativeCommandError`. Cargo writes progress and warnings to stderr, so red `Compiling`, warnings, or Rustup informational lines are not proof of failure. The authoritative signal is `$LASTEXITCODE` and Cargo’s final line:

```powershell
cargo build --locked --manifest-path prototypes/slint-shell/Cargo.toml --bin slint-shell
if ($LASTEXITCODE -ne 0) { throw "cargo failed with exit code $LASTEXITCODE" }
```

Use PowerShell 7 where possible for UTF-8 logs. In Windows PowerShell 5.1, Cyrillic and emoji can appear as mojibake even when the application sent the correct UTF-16 text. Verify the resulting text in Notepad and save logs explicitly as UTF-8; do not diagnose Unicode behavior from a mojibake console line alone.

## Running the Slint prototype

Use Skia for the Windows visual pass:

```powershell
$env:SLINT_BACKEND = 'winit-skia'
$env:RUST_LOG = 'debug'
& C:\lhc\prototypes\slint-shell\target\release\slint-shell.exe
```

`winit-software` built and rendered the UI, but Windows showed monochrome outline emoji. `winit-skia` rendered the normal color Windows emoji and is the Windows candidate renderer. Some squares on the stress page were caused by the prototype generating contiguous Unicode code-point ranges that include unassigned or non-emoji symbols; they are test-data defects unless the same known-valid emoji also fails.

The server starts hidden and remains accessible from the tray. A second invocation is an IPC client:

```powershell
$exe = 'C:\lhc\prototypes\slint-shell\target\release\slint-shell.exe'
& $exe ping
& $exe show settings
& $exe show emoji
& $exe show quick
& $exe hide
& $exe quit
```

A connection-refused response means the server is not running or has not finished startup. It is not a popup rendering failure. Wait for `ping` to succeed before issuing UI commands.

## Automated smoke tests

The portable `interactions` example exercises clipboard, keyboard input, modal behavior, DnD, cancellation, edge scrolling, theme, locale, and hidden popups:

```powershell
$env:SLINT_BACKEND = 'winit-software'
cargo run --locked --manifest-path prototypes/slint-shell/Cargo.toml --example interactions
if ($LASTEXITCODE -ne 0) { throw 'interaction scenario failed' }
```

For IPC lifecycle testing, start one server with `Start-Process`, poll `ping` until ready, alternate `show emoji`/`show quick` and `hide` 100 times, then send `quit`. Validate each client’s `$LASTEXITCODE`, confirm that the server exits within a timeout, and inspect redirected stdout/stderr. Do not depend solely on `$server.ExitCode`: PowerShell returned an empty value for the asynchronously launched process in the first harness even though all 100 cycles and shutdown completed successfully.

GUI automation does not replace the manual pass. Tray menus, actual foreground focus, global hotkeys, DPI transitions, emoji rendering, and insertion into unrelated applications must be observed in an interactive desktop.

## Global hotkeys without a Scroll Lock key

`global-hotkey` registers F13 and Scroll Lock with Windows. `keybd_event` or `SendInput` from inside Windows is not a valid test of this path: Windows can distinguish injected input, and an injected Scroll Lock did not trigger the registered hotkey in the pilot.

Use the QEMU monitor to inject an emulated keyboard key instead:

```bash
vm_dir=/mnt/disk2/vm/left-hand-control-windows
printf 'sendkey scroll_lock\n' | socat - UNIX-CONNECT:"$vm_dir/qemu-monitor.sock"
```

Run this while Notepad, Windows Terminal, and a browser field are foreground. Confirm that Quick opens on the first event. Use a real keyboard or QEMU hardware-level injection for the final hotkey verdict; mark a `SendInput`-only attempt as inconclusive, not failed.

## Focus return and text insertion

Current pilot status as of 2026-09-22: delayed IPC invocation returned focus to
Notepad and inserted exact Quick text and one emoji through the clipboard fallback;
the previous text clipboard was restored. Invocation from the system tray still
failed to return focus to Notepad. The Windows pass is paused on that blocker.

For Emoji and Quick, repeat the following with Notepad, Windows Terminal, and a browser:

1. Type a unique `BEFORE:` marker and leave the caret at the end.
2. Open the popup from the tray, IPC, and a hardware-level global hotkey.
3. Select by mouse and separately by keyboard.
4. Confirm that the popup closes and the original window becomes foreground.
5. Compare the inserted value character-for-character.

Test Latin, Cyrillic, punctuation, a BMP emoji, a surrogate-pair emoji, and a composed emoji sequence. Also test Escape and focus loss, which must insert nothing.

The Slint pilot exposed a real Windows defect here: `enigo 0.6.1` has an incorrect UTF-16 key-up value for surrogate pairs, and direct Unicode `SendInput` still raced foreground activation. Results included `Действие 02` becoming a truncated prefix followed by repeated `2` characters. Increasing arbitrary sleeps or treating `SetForegroundWindow` success as proof that the target can already consume input is not a reliable fix. Keep this case in the regression suite.

If clipboard paste is used as the Windows fallback, verify all of the following:

- the exact Unicode value is inserted once;
- the previous text clipboard is restored only after the target processes Paste;
- behavior for non-text clipboard formats is defined and tested;
- a user focus change during the operation cannot paste into a different application;
- Ctrl, Alt, Shift, and Win are not left logically pressed.

The complete Slint checklist is in [Windows Slint validation](../dev_docs/slint-windows-validation.md). Product-level mapper, layouts, macros, and `text:` validation are in the [cross-platform validation plan](../dev_docs/cross-platform-product-validation-plan.md).

## Snapshots and result recording

Keep at least these restore points:

- `windows-clean`: Windows desktop and local user, before developer tools;
- `windows-toolchain`: MSVC, Rust, Node/pnpm and WebView2 installed;
- `slint-ready`: exact tested source built and ready to run.

Before recording a result, capture:

- Windows edition, version, and build number;
- qcow2 snapshot name;
- repository commit and local patch list;
- `rustc --version`, `cargo --version`, Slint version, backend, and renderer;
- VM CPU/RAM/display scale;
- exact command and whether the binary is debug or release;
- logs, screenshots, passed attempts/total attempts, and known limitations.

VM results establish functional compatibility. Final claims about physical media keys, multiple keyboards, USB hotplug, sleep/wake, multiple monitors, driver behavior, and user-perceived latency require a physical Windows machine.

## Troubleshooting summary

| Symptom | Meaning and action |
| --- | --- |
| Windows setup requires Secure Boot | Use Secure Boot OVMF code and variables, `smm=on`, secure pflash, and TPM 2.0; recreate the VM configuration rather than bypassing the check. |
| Work/school sign-in is mandatory | Disconnect the VM network during OOBE and choose the image’s local/domain account path; never use a real organization account. |
| Host text cannot be pasted into Windows | Expected with the GTK display and no guest agent; transfer a script over `http://10.0.2.2:<port>`. |
| `winget` reports `0x8a15005e` for `msstore` | Add `--source winget` and check the native exit code. |
| Cargo/Rustup output is red | PowerShell is styling stderr; use `$LASTEXITCODE`, not color, as the verdict. |
| Rust E0282 in global-hotkey/tray handlers | Give the callback argument its explicit event type; ensure the guest has the patched source rather than a stale archive. |
| Build pauses after warnings | Release LTO can be CPU/RAM intensive; inspect activity and wait before declaring a hang. |
| Emoji are monochrome | Confirm `SLINT_BACKEND=winit-skia`; software rendering did not provide the desired Windows color emoji result. |
| Only some stress emoji are squares | Check whether the code point is a valid supported emoji before blaming the renderer. |
| IPC says connection refused | Start the persistent server and wait for `ping`; a CLI invocation alone cannot contact a stopped server. |
| Synthetic Scroll Lock does nothing | Use QEMU monitor `sendkey` or physical hardware; in-guest `SendInput` is not conclusive for a registered global hotkey. |
| Quick inserts repeated final digits | Reproduce with the exact mixed-language string; investigate foreground readiness and Unicode/clipboard insertion, not just popup rendering. |
| `qemu-img snapshot` reports a write lock | Shut down the guest and wait for QEMU to exit before manipulating the qcow2 file. |
