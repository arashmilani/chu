import { render, fireEvent } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { useRef } from "react";

import { useFocusTrap } from "../hooks/useFocusTrap";
import { usePopoverPosition } from "../hooks/usePopoverPosition";

function Trap({ children }: { children: React.ReactNode }) {
  const ref = useRef<HTMLDivElement | null>(null);
  useFocusTrap(ref);
  return (
    <div ref={ref} data-testid="trap">
      {children}
    </div>
  );
}

describe("useFocusTrap", () => {
  it("wraps focus from the last back to the first on Tab", () => {
    const { getByTestId, getByText } = render(
      <Trap>
        <button>first</button>
        <button>last</button>
      </Trap>,
    );
    const last = getByText("last");
    last.focus();
    fireEvent.keyDown(getByTestId("trap"), { key: "Tab" });
    expect(document.activeElement).toBe(getByText("first"));
  });

  it("wraps focus from the first back to the last on Shift+Tab", () => {
    const { getByTestId, getByText } = render(
      <Trap>
        <button>first</button>
        <button>last</button>
      </Trap>,
    );
    const first = getByText("first");
    first.focus();
    fireEvent.keyDown(getByTestId("trap"), { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(getByText("last"));
  });
});

function PositionedConsumer({ x, y }: { x: number; y: number }) {
  const pos = usePopoverPosition({ x, y }, { width: 200, height: 100 });
  return <div data-testid="pos">{pos ? `${pos.left},${pos.top}` : "null"}</div>;
}

describe("usePopoverPosition", () => {
  it("anchors centered on the x coordinate", () => {
    const { getByTestId } = render(<PositionedConsumer x={500} y={50} />);
    const text = getByTestId("pos").textContent;
    // x=500, width=200 -> left = 400
    expect(text).toMatch(/^400,/);
  });

  it("clamps inside the viewport with a margin", () => {
    const { getByTestId } = render(<PositionedConsumer x={5} y={5} />);
    const text = getByTestId("pos").textContent;
    // x=5 would yield left=-95; clamp to margin=8.
    expect(text).toBe("8,8");
  });
});
