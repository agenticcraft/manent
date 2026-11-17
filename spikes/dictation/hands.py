# Virtual keyboard in the isolated session through Mutter's own RemoteDesktop D-Bus API (no consent there).
from gi.repository import Gio, GLib
BUS = "org.gnome.Mutter.RemoteDesktop"
class Hands:
    def __init__(self):
        self.c = Gio.bus_get_sync(Gio.BusType.SESSION)
        path = self.c.call_sync(BUS, "/org/gnome/Mutter/RemoteDesktop", BUS, "CreateSession", None, GLib.VariantType("(o)"), 0, -1).unpack()[0]
        self.p = path
        try: self.call("Enable", GLib.Variant("(a{sv})", ({},)))
        except Exception as e: print("enable before start:", e)
        self.call("Start")
    def call(self, m, args=None, rt=None):
        return self.c.call_sync(BUS, self.p, BUS + ".Session", m, args, GLib.VariantType(rt) if rt else None, 0, -1)
    def key(self, keysym, down):
        self.call("NotifyKeyboardKeysym", GLib.Variant("(ub)", (keysym, down)))
    def combo(self, *keysyms):
        for k in keysyms: self.key(k, True)
        for k in reversed(keysyms): self.key(k, False)
    def introspect(self):
        return self.c.call_sync(BUS, self.p, "org.freedesktop.DBus.Introspectable", "Introspect", None, GLib.VariantType("(s)"), 0, -1).unpack()[0]
CTRL, SHIFT, ALT, V, RET, TAB = 0xffe3, 0xffe1, 0xffe9, 0x76, 0xff0d, 0xff09

import secrets
def portal_request(method, iface, args_fn, timeout=20):
    """Call a portal method that answers through a Request object; return (code, results)."""
    c = Gio.bus_get_sync(Gio.BusType.SESSION)
    token = "t" + secrets.token_hex(4)
    sender = c.get_unique_name()[1:].replace(".", "_")
    handle = f"/org/freedesktop/portal/desktop/request/{sender}/{token}"
    loop = GLib.MainLoop(); res = {}
    def on_resp(conn, snd, path, i, sig, params, *_):
        res["v"] = params.unpack(); loop.quit()
    sid = c.signal_subscribe("org.freedesktop.portal.Desktop", "org.freedesktop.portal.Request", "Response", handle, None, 0, on_resp)
    c.call_sync("org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop", iface, method, args_fn(token), None, 0, -1)
    GLib.timeout_add_seconds(timeout, loop.quit); loop.run()
    c.signal_unsubscribe(sid)
    return res.get("v")

def shot(path):
    """Screenshot of the isolated session through the portal (permission granted in its own store)."""
    c = Gio.bus_get_sync(Gio.BusType.SESSION)
    try:
        c.call_sync("org.freedesktop.impl.portal.PermissionStore", "/org/freedesktop/impl/portal/PermissionStore",
            "org.freedesktop.impl.portal.PermissionStore", "SetPermission",
            GLib.Variant("(sbssas)", ("screenshot", True, "screenshot", "", ["yes"])), None, 0, -1)
    except Exception as e: print("perm:", e)
    r = portal_request("Screenshot", "org.freedesktop.portal.Screenshot",
        lambda t: GLib.Variant("(sa{sv})", ("", {"handle_token": GLib.Variant("s", t), "interactive": GLib.Variant("b", False)})))
    if r and r[0] == 0:
        import shutil, urllib.parse
        shutil.move(urllib.parse.unquote(r[1]["uri"][7:]), path)
    return r

def mutter_eis(hands):
    """An EIS fd from Mutter's own RemoteDesktop session."""
    r, fds = hands.c.call_with_unix_fd_list_sync("org.gnome.Mutter.RemoteDesktop", hands.p,
        "org.gnome.Mutter.RemoteDesktop.Session", "ConnectToEIS", GLib.Variant("(a{sv})", ({},)),
        GLib.VariantType("(h)"), 0, -1, None, None)
    return fds.get(r.unpack()[0])
