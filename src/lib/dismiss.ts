import { useEffect, type RefObject } from "react";

/**
 * Close a popover or menu on a press outside it or on Escape (VRT-42). Pressing inside, for
 * example on its own toggle when `ref` wraps the toggle too, does nothing.
 */
export function useDismiss(ref: RefObject<HTMLElement | null>, open: boolean, onClose: () => void) {
  useEffect(() => {
    if (!open) return;
    const outside = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const escape = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("keydown", escape);
    };
  }, [ref, open, onClose]);
}
