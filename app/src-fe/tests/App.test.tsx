import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import App from "../App";

vi.mock("../ipc", () => import("../ipc/__mocks__"));

describe("App", () => {
  it("renders the Settings window — the only screen in the app", () => {
    render(<App />);
    // Settings exposes a tablist; that's the unmistakable signature.
    expect(
      screen.getByRole("tablist", { name: /settings sections/i }),
    ).toBeInTheDocument();
  });
});
