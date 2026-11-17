# A bare GTK4 VTE terminal running cat (VTE is the terminal widget in Ptyxis and GNOME Terminal).
import gi
gi.require_version("Gtk", "4.0"); gi.require_version("Vte", "3.91")
from gi.repository import Gtk, Vte, GLib
def activate(app):
    win = Gtk.ApplicationWindow(application=app, title="vte"); win.set_default_size(800, 400)
    t = Vte.Terminal(); win.set_child(t)
    t.spawn_async(Vte.PtyFlags.DEFAULT, None, ["/usr/bin/cat"], None, GLib.SpawnFlags.DEFAULT, None, None, -1, None, None, None)
    win.present(); t.grab_focus()
app = Gtk.Application(application_id="test.vte"); app.connect("activate", activate); app.run([])
