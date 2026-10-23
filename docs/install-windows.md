# Windows 11 x64: portable distribution

Install the latest [Microsoft Visual C++ v14 Redistributable (x64)](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)
if it is not already installed ([official x64 installer](https://aka.ms/vc14/vc_redist.x64.exe)).
The executable requires MSVCP140.dll and VCRUNTIME140.dll. Rust, Visual Studio
and the Windows SDK are not required to run the application.

Extract the complete ZIP into a directory writable by your user, then run
`left-hand-control.exe` without administrator privileges. The application starts
hidden in the system tray. Open Settings from the tray or run
`left-hand-control.exe show settings`. A second launch opens the existing instance.

The ZIP is portable in the sense that it needs no installer or service. Settings
are stored in your Windows user profile, not beside the executable. Moving the
folder does not move or erase your settings. Configure the keyboard mapper in
Settings; Windows interception applies to the interactive session.

To update, quit from the tray (or run `left-hand-control.exe quit`), extract the
new ZIP into a new folder and start its executable. Keep the old folder until
you verify the update. To uninstall, quit and delete the extracted folder.
User configuration is retained. Settings → "Launch on startup" registers the
running executable under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`;
each launch points the entry at the current executable, so start the new folder's
executable once after an update. Disabling startup in Task Manager is respected
and turns the setting off. Turn the setting off before deleting the folder.
This distribution installs no service and does not enable startup by itself.

Verify the ZIP with PowerShell `Get-FileHash -Algorithm SHA256 <archive.zip>` and
compare it with `WINDOWS-SHA256SUMS` from the same release. The executable is not
Authenticode-signed; SHA256 checks integrity and does not replace publisher signing.
Source code is supplied by the matching GitHub release source archive; the release
also supplies the dependency vendor archive. License: GPL-3.0-or-later, see LICENSE.

Automated acceptance covers Windows 11 x64 in a non-elevated interactive VM,
including release mapper, Unicode popups, focus return and crash recovery.
Physical keyboards, lock/unlock, sleep/wake, multiple monitors and DPI, and input
into browsers/IDEs/terminals require separate acceptance. Elevated applications
and the secure desktop are outside the accepted scope. Windows 10 and Windows ARM
are not covered. See `docs/testing/windows.md` in the source tree for exact results.
