export interface PopoverAnchor {
  x: number;
  y: number;
}

export interface PopoverPosition {
  top: number;
  left: number;
}

// Anchor a popover element to a coordinate, keeping it inside the
// viewport. Pure derivation — no effect needed; React 19 reruns the
// hook on prop change just fine.
export function usePopoverPosition(
  anchor: PopoverAnchor | null,
  size: { width: number; height: number },
): PopoverPosition | null {
  if (!anchor) return null;

  const viewportW =
    typeof window === "undefined" ? size.width : window.innerWidth;
  const viewportH =
    typeof window === "undefined" ? size.height : window.innerHeight;

  let left = anchor.x - size.width / 2;
  let top = anchor.y;

  const margin = 8;
  left = Math.max(margin, Math.min(left, viewportW - size.width - margin));
  top = Math.max(margin, Math.min(top, viewportH - size.height - margin));
  return { top, left };
}
