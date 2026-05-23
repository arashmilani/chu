import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import App, { readWindowKind } from "../App";

vi.mock("../ipc", () => import("../ipc/__mocks__"));

describe("readWindowKind", () => {
  it("returns popover when no query param", () => {
    expect(readWindowKind("")).toBe("popover");
  });
  it("returns editor for ?window=editor", () => {
    expect(readWindowKind("?window=editor")).toBe("editor");
  });
  it("returns settings for ?window=settings", () => {
    expect(readWindowKind("?window=settings")).toBe("settings");
  });
  it("falls back to popover for unknown values", () => {
    expect(readWindowKind("?window=junk")).toBe("popover");
  });
});

describe("App", () => {
  it("renders the default popover when no window query param is set", () => {
    render(<App />);
    expect(
      screen.getByRole("dialog", { name: /mira controller/i }),
    ).toBeInTheDocument();
  });
});
