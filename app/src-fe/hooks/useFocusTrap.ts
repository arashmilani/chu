import { useEffect } from "react";

// Trap Tab focus inside `ref.current` while the container is mounted.
// Picks up the first/last tabbable element on every key press so it
// works for dynamically-rendered children.
export function useFocusTrap(
  ref: React.RefObject<HTMLElement | null>,
  enabled: boolean = true,
): void {
  useEffect(() => {
    if (!enabled) return;
    const node = ref.current;
    if (!node) return;

    function handle(e: KeyboardEvent) {
      if (e.key !== "Tab" || !node) return;
      const focusables = node.querySelectorAll<HTMLElement>(
        'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      );
      if (focusables.length === 0) {
        e.preventDefault();
        return;
      }
      const first = focusables[0];
      const last = focusables[focusables.length - 1];
      const active = document.activeElement as HTMLElement | null;
      if (e.shiftKey && active === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    }

    node.addEventListener("keydown", handle);
    return () => node.removeEventListener("keydown", handle);
  }, [ref, enabled]);
}
