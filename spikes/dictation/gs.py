# GlobalShortcuts portal client: bind one shortcut, log Activated/Deactivated with times.
import os, sys, time, threading
sys.path.insert(0, os.path.dirname(__file__))
from hands import *
from gi.repository import Gio, GLib
GS = "org.freedesktop.portal.GlobalShortcuts"
def bind(trigger="CTRL+SHIFT+space"):
    c = Gio.bus_get_sync(Gio.BusType.SESSION)
    c.call_sync("org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop", "org.freedesktop.host.portal.Registry",
                "Register", GLib.Variant("(sa{sv})", ("test.manent.probe", {})), None, 0, -1)
    r = portal_request("CreateSession", GS, lambda t: GLib.Variant("(a{sv})", ({"handle_token": GLib.Variant("s", t), "session_handle_token": GLib.Variant("s", "s" + t)},)))
    session = r[1]["session_handle"]
    events = []
    def on(conn, snd, path, i, sig, params, *_):
        events.append((sig, params.unpack()[1], time.monotonic()))
    for s in ("Activated", "Deactivated"):
        c.signal_subscribe("org.freedesktop.portal.Desktop", GS, s, "/org/freedesktop/portal/desktop", None, 0, on)
    sc = [("dictate", {"description": GLib.Variant("s", "Dictate"), "preferred_trigger": GLib.Variant("s", trigger)})]
    def do_bind():
        return portal_request("BindShortcuts", GS, lambda t: GLib.Variant("(oa(sa{sv})sa{sv})", (session, sc, "", {"handle_token": GLib.Variant("s", t)})), timeout=15)
    return c, session, events, do_bind
