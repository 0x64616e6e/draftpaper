"""Lowering: the shell's unmanaged windows (margins, title block) go under every i3 window.

  draftpaper lower W H [PID]   push the override-redirect windows of this size (±2 px) owned by PID
                               (default: the parent process, the shell that runs this) to the bottom of
                               the stack, so they read as part of the paper and fullscreen windows cover
                               them. Waits up to a second for the window to be mapped.
"""
import os, sys, time
from Xlib import X, display

def main(a):
    W, H = int(a[0]), int(a[1])
    PID = int(a[2]) if len(a) > 2 else os.getppid()
    d = display.Display()
    root = d.screen().root
    PID_ATOM = d.intern_atom("_NET_WM_PID")
    for _ in range(20):
        hits = []
        for w in root.query_tree().children:
            try:
                attrs = w.get_attributes()
                if not attrs.override_redirect or attrs.map_state != X.IsViewable:
                    continue
                g = w.get_geometry()
                pid = w.get_full_property(PID_ATOM, X.AnyPropertyType)
                if abs(g.width - W) <= 2 and abs(g.height - H) <= 2 and pid and pid.value[0] == PID:
                    hits.append(w)
            except Exception:
                pass
        if hits:
            for w in hits:
                w.configure(stack_mode=X.Below)
            d.sync()
            return 0
        time.sleep(0.05)
    sys.exit(f"draftpaper lower: no unmanaged window of {W}x{H} from process {PID}")
