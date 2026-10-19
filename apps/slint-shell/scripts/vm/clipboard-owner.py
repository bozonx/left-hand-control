#!/usr/bin/env python3
"""Native GTK clipboard owner for the disposable GNOME acceptance guest."""
import argparse
from pathlib import Path

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, GLib, Gtk

parser = argparse.ArgumentParser()
parser.add_argument("--paste", choices=["replace", "hold"])
args = parser.parse_args()
app = Gtk.Application(application_id="dev.bozonx.lhc-e2e-clipboard" + ("." + args.paste if args.paste else ""))


def activate(application):
    window = Gtk.ApplicationWindow(application=application, title="LHC clipboard fixture")
    entry = Gtk.Entry()
    entry.set_placeholder_text("Clipboard preservation fixture")
    window.set_child(entry)
    window.set_default_size(500, 150)
    application.fixture_window = window
    providers = [
        Gdk.ContentProvider.new_for_bytes(
            "text/plain;charset=utf-8", GLib.Bytes.new(b"lhc-before-unicode")
        ),
        Gdk.ContentProvider.new_for_bytes(
            "application/x-lhc-e2e", GLib.Bytes.new(b"binary\x00\xffpayload")
        ),
    ]
    provider = Gdk.ContentProvider.new_union(providers)
    application.fixture_provider = provider
    seeded = False

    def seed(*_args):
        nonlocal seeded
        if window.is_active() and not seeded:
            if not args.paste:
                window.get_clipboard().set_content(provider)
            seeded = True
            (Path.home() / "lhc-e2e/clipboard-ready").touch()

    def key_pressed(_controller, keyval, _keycode, modifiers):
        paste = (keyval in (Gdk.KEY_v, Gdk.KEY_V) and modifiers & Gdk.ModifierType.CONTROL_MASK) or (
            keyval == Gdk.KEY_Insert and modifiers & Gdk.ModifierType.SHIFT_MASK
        )
        if args.paste and paste:
            if args.paste == "replace":
                replacement = Gdk.ContentProvider.new_for_bytes(
                    "text/plain;charset=utf-8", GLib.Bytes.new(b"lhc-new-owner")
                )
                application.fixture_provider = replacement
                window.get_clipboard().set_content(replacement)
            (Path.home() / "lhc-e2e/clipboard-paste-received").touch()
            return True
        return False

    controller = Gtk.EventControllerKey.new()
    controller.set_propagation_phase(Gtk.PropagationPhase.CAPTURE)
    controller.connect("key-pressed", key_pressed)
    entry.add_controller(controller)
    window.connect("notify::is-active", seed)
    window.present()
    entry.grab_focus()


app.connect("activate", activate)
app.run([])
