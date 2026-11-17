# RemoteDesktop portal session with keyboard + clipboard, as Manent would use it for auto-paste.
import os, sys, time
sys.path.insert(0, os.path.dirname(__file__))
from hands import *
from gi.repository import Gio, GLib
RD, CB = "org.freedesktop.portal.RemoteDesktop", "org.freedesktop.portal.Clipboard"
DEST, OBJ = "org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop"
class Portal:
    def __init__(self, restore_token=None, on_dialog=None):
        self.c = Gio.bus_get_sync(Gio.BusType.SESSION)
        self.c.call_sync(DEST, OBJ, "org.freedesktop.host.portal.Registry", "Register",
                         GLib.Variant("(sa{sv})", ("test.manent.probe", {})), None, 0, -1)
        V = GLib.Variant
        r = portal_request("CreateSession", RD, lambda t: V("(a{sv})", ({"handle_token": V("s", t), "session_handle_token": V("s", "s" + t)},)))
        self.session = r[1]["session_handle"]
        opts = lambda t: {"handle_token": V("s", t), "types": V("u", 1), "persist_mode": V("u", 2), **({"restore_token": V("s", restore_token)} if restore_token else {})}
        r = portal_request("SelectDevices", RD, lambda t: V("(oa{sv})", (self.session, opts(t))))
        assert r[0] == 0, r
        self.c.call_sync(DEST, OBJ, CB, "RequestClipboard", V("(oa{sv})", (self.session, {})), None, 0, -1)
        if on_dialog: on_dialog()
        t0 = time.monotonic()
        r = portal_request("Start", RD, lambda t: V("(osa{sv})", (self.session, "", {"handle_token": V("s", t)})), timeout=30)
        self.start_ms = (time.monotonic() - t0) * 1000
        self.start = r
        self.text = None
        self.c.signal_subscribe(DEST, CB, "SelectionTransfer", OBJ, None, 0, self._transfer)
    def eis_fd(self):
        r, fds = self.c.call_with_unix_fd_list_sync(DEST, OBJ, RD, "ConnectToEIS", GLib.Variant("(oa{sv})", (self.session, {})),
                                                    GLib.VariantType("(h)"), 0, -1, None, None)
        return fds.get(r.unpack()[0])
    def set_clipboard(self, text):
        self.text = text
        self.c.call_sync(DEST, OBJ, CB, "SetSelection", GLib.Variant("(oa{sv})", (self.session, {"mime_types": GLib.Variant("as", ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING"])})), None, 0, -1)
    def _transfer(self, conn, snd, path, i, sig, params, *_):
        session, mime, serial = params.unpack()
        r, fds = self.c.call_with_unix_fd_list_sync(DEST, OBJ, CB, "SelectionWrite", GLib.Variant("(ou)", (session, serial)),
                                                    GLib.VariantType("(h)"), 0, -1, None, None)
        fd = fds.get(r.unpack()[0]); os.write(fd, self.text.encode()); os.close(fd)
        self.c.call_sync(DEST, OBJ, CB, "SelectionWriteDone", GLib.Variant("(oub)", (session, serial, True)), None, 0, -1)
    def close(self):
        self.c.call_sync(DEST, self.session, "org.freedesktop.portal.Session", "Close", None, None, 0, -1)
