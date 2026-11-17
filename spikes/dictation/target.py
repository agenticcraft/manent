# A GTK4 window with one text field; every change is written to argv[1].
import sys, gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk
out = sys.argv[1]
def activate(app):
    win = Gtk.ApplicationWindow(application=app, title="target")
    win.set_default_size(600, 300)
    view = Gtk.TextView()
    buf = view.get_buffer()
    buf.connect("changed", lambda b: open(out, "w").write(b.get_text(b.get_start_iter(), b.get_end_iter(), False)))
    win.set_child(view)
    win.present()
    view.grab_focus()
app = Gtk.Application(application_id="test.target")
app.connect("activate", activate)
app.run([])
