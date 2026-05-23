import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

import { Editor } from "../windows/Editor";

vi.mock("../ipc", () => import("../ipc/__mocks__"));

const builtIn = {
  id: "coding",
  name: "Coding",
  builtIn: true,
  hotkey: null,
  settings: {
    refreshMode: "a2" as const,
    speed: 6,
    contrast: 12,
    ditherMode: 0,
    whiteFilter: 16,
    blackFilter: 8,
    coldLight: 0,
    warmLight: 0,
  },
  createdAt: "1970-01-01T00:00:00Z",
  modifiedAt: "1970-01-01T00:00:00Z",
};

const custom = {
  ...builtIn,
  id: "550e8400-e29b-41d4-a716-446655440000",
  name: "My Coding",
  builtIn: false,
};

describe("Editor action bar", () => {
  beforeEach(async () => {
    const mod = await import("../ipc/__mocks__");
    mod.__setProfiles([builtIn, custom]);
  });

  it("always offers a Duplicate button for the selected profile", async () => {
    render(<Editor initialProfiles={[builtIn, custom]} />);
    expect(
      await screen.findByRole("button", { name: /^duplicate$/i }),
    ).toBeInTheDocument();
  });

  it("shows Reset-to-defaults on built-in presets and Delete on custom ones", async () => {
    const { rerender } = render(<Editor initialProfiles={[builtIn, custom]} />);
    expect(
      await screen.findByRole("button", { name: /reset to defaults/i }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /^delete$/i }),
    ).not.toBeInTheDocument();

    // Switch to the custom profile.
    rerender(<Editor initialProfiles={[builtIn, custom]} />);
    fireEvent.click(screen.getByRole("button", { name: /my coding/i }));
    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /^delete$/i }),
      ).toBeInTheDocument();
    });
    expect(
      screen.queryByRole("button", { name: /reset to defaults/i }),
    ).not.toBeInTheDocument();
  });

  it("does NOT disable the sliders on a built-in preset (they're editable now)", async () => {
    render(<Editor initialProfiles={[builtIn, custom]} />);
    await screen.findByRole("button", { name: /reset to defaults/i });
    const speed = screen.getByRole("slider", {
      name: /refresh speed/i,
    }) as HTMLInputElement;
    expect(speed.disabled).toBe(false);
  });
});
