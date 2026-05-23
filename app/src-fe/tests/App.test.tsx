import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import App from "../App";

vi.mock("../ipc", () => import("../ipc/__mocks__"));

describe("App", () => {
  it("renders the wordmark", () => {
    render(<App />);
    expect(
      screen.getByRole("heading", { name: /mira controller/i }),
    ).toBeInTheDocument();
  });
});
