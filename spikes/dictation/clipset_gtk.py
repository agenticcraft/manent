# Set the clipboard from a GTK app that has no focused window (argv: gtk version, text).
import sys, gi
gi.require_version("Gtk", sys.argv[1]); gi.require_version("Gdk", sys.argv[1])
from gi.repository import Gtk, Gdk, GLib
text = sys.argv[2]
if sys.argv[1] == "4.0":
    Gdk.Display.get_default().get_clipboard().set(text)
else:
    Gtk.init(); Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).set_text(text, -1)
GLib.timeout_add(4000, lambda: sys.exit(0)); GLib.MainLoop().run()
