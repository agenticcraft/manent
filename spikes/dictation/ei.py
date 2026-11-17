# Minimal libei sender over ctypes: keyboard keys through an EIS fd (from Mutter or the RemoteDesktop portal).
import ctypes, select, time
L = ctypes.CDLL("libei.so.1")
P = ctypes.c_void_p
for name, res, args in [
    ("ei_new_sender", P, [P]), ("ei_configure_name", None, [P, ctypes.c_char_p]),
    ("ei_setup_backend_fd", ctypes.c_int, [P, ctypes.c_int]), ("ei_get_fd", ctypes.c_int, [P]),
    ("ei_dispatch", None, [P]), ("ei_get_event", P, [P]), ("ei_event_get_type", ctypes.c_int, [P]),
    ("ei_event_unref", P, [P]), ("ei_event_get_seat", P, [P]), ("ei_event_get_device", P, [P]),
    ("ei_seat_bind_capabilities", None, [P, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int]),
    ("ei_device_ref", P, [P]), ("ei_device_has_capability", ctypes.c_bool, [P, ctypes.c_int]),
    ("ei_device_start_emulating", None, [P, ctypes.c_uint32]), ("ei_device_stop_emulating", None, [P]),
    ("ei_device_keyboard_key", None, [P, ctypes.c_uint32, ctypes.c_bool]),
    ("ei_device_frame", None, [P, ctypes.c_uint64]), ("ei_now", ctypes.c_uint64, [P]),
    ("ei_event_type_to_string", ctypes.c_char_p, [ctypes.c_int]),
]:
    f = getattr(L, name); f.restype = res; f.argtypes = args
CAP_KEYBOARD = 1 << 2  # EI_DEVICE_CAP_KEYBOARD
EV_SEAT_ADDED, EV_DEVICE_ADDED, EV_DEVICE_RESUMED = 3, 5, 8

class Keyboard:
    def __init__(self, fd, timeout=5):
        self.ei = L.ei_new_sender(None); L.ei_configure_name(self.ei, b"manent-probe")
        assert L.ei_setup_backend_fd(self.ei, fd) == 0
        self.dev = None; self.seq = 1; self.log = []
        end = time.time() + timeout
        while self.dev is None and time.time() < end:
            self.pump(0.2)
        assert self.dev, f"no keyboard device; events {self.log}"
    def pump(self, wait):
        select.select([L.ei_get_fd(self.ei)], [], [], wait)
        L.ei_dispatch(self.ei)
        while True:
            ev = L.ei_get_event(self.ei)
            if not ev: break
            t = L.ei_event_get_type(ev); self.log.append(L.ei_event_type_to_string(t).decode())
            if t == EV_SEAT_ADDED:
                L.ei_seat_bind_capabilities(L.ei_event_get_seat(ev), CAP_KEYBOARD, 0, 0, 0)
            elif t == EV_DEVICE_RESUMED:
                d = L.ei_event_get_device(ev)
                if L.ei_device_has_capability(d, CAP_KEYBOARD):
                    self.dev = L.ei_device_ref(d)
                    L.ei_device_start_emulating(self.dev, self.seq); self.seq += 1
            L.ei_event_unref(ev)
    def key(self, keycode, down):
        L.ei_device_keyboard_key(self.dev, keycode, down)
        L.ei_device_frame(self.dev, L.ei_now(self.ei)); self.pump(0.01)
    def tap(self, *codes):
        for c in codes: self.key(c, True)
        for c in reversed(codes): self.key(c, False)
# evdev keycodes
KEY = dict(ctrl=29, shift=42, v=47, a=30, b=48, esc=1, enter=28, tab=15, space=57)
