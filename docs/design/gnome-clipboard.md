# GNOME clipboard: use the supported portal

Decision after the text-input audit and a disposable GNOME 50.1 VM prototype:
keep clipboard access in the Rust Clipboard portal backend. Do not add native
libraries to the Shell extension or extend its D-Bus API for clipboard ownership.

A proposed Shell-owned multi-MIME source would have decoupled restored clipboard
data from RemoteDesktop. Native Mutter exposes the necessary selection-source
virtual methods, but the GNOME 50 GJS prototype failed when registering the class:

```
VFunc read_async accepts another callback as a parameter. This is not supported
```

`Meta.SelectionSourceMemory` alone handles a single MIME format, so it cannot
implement the required lossless preservation of an arbitrary offer. A native
GObject shim could bridge this gap, but loading a binary into Shell would add a
Mutter ABI maintenance burden. GNOME's extension review rules prohibit shipping
binary libraries and strongly discourage external binaries. This is not the
integration path chosen for this application.

Use RemoteDesktop/libei for keyboard events and the Clipboard portal, requested
before Start, for clipboard access. Request permission explicitly, retain tokens
in the appropriate scope and allow the user to reset them. Clipboard failure must
not disable otherwise available keyboard typing. Keep source preservation bounded
and all-or-nothing, check destination and cancellation, use the same reader grace
policy as KDE and wait for signals while idle.

The portal clipboard is attached to its session. While that session owns restored
MIME data, retaining it can retain GNOME's remote-control indicator. Retiring a
keyboard backend must not erase the user's clipboard merely to hide the indicator.
Close the retired clipboard session after another owner replaces its data. A new
input backend may overlap this retired owner; ConnectToEIS is allowed only once
per session and Notify methods cannot be mixed into an EIS session, so arbitrary
reuse across input mechanisms would violate the portal contract.

Fully independent lifetime with guaranteed preservation of all formats needs a
supported upstream clipboard/persistence API. Do not emulate data-control through
Shell internals, silently restore only text, inject hidden focus windows, or claim
that closing RemoteDesktop cannot lose the selection.

References:

- [GNOME extension scripts/binaries rules](https://gjs.guide/extensions/review-guidelines/review-guidelines.html#scripts-and-binaries)
- [Clipboard portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Clipboard.html)
- [RemoteDesktop cross-portal integration](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#cross-portal-integration)
- [ConnectToEIS session restrictions](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#connecttoeis)
