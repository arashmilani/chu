import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

import { Popover } from "../windows/Popover";

vi.mock("../ipc", () => import("../ipc/__mocks__"));

const sampleProfiles = [
  {
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
  },
];

describe("First-run flow inside the Popover", () => {
  beforeEach(async () => {
    const mod = await import("../ipc/__mocks__");
    mod.__setProfiles(sampleProfiles);
    mod.__setStatus({ connected: true });
    mod.__setFirstRun(true);
  });

  it("shows the welcome dialog instead of the popover on first launch", async () => {
    render(
      <Popover
        initialProfiles={sampleProfiles}
        initialStatus={{ connected: true }}
      />,
    );
    expect(
      await screen.findByRole("dialog", {
        name: /welcome to mira controller/i,
      }),
    ).toBeInTheDocument();
    // The regular popover dialog is NOT shown.
    expect(
      screen.queryByRole("dialog", { name: /^mira controller$/i }),
    ).not.toBeInTheDocument();
  });

  it("walks the three steps and shows the regular popover after dismissal", async () => {
    render(
      <Popover
        initialProfiles={sampleProfiles}
        initialStatus={{ connected: true }}
      />,
    );
    await screen.findByRole("dialog", {
      name: /welcome to mira controller/i,
    });

    fireEvent.click(screen.getByRole("button", { name: /continue/i }));
    await screen.findByRole("heading", { name: /device detected/i });
    fireEvent.click(screen.getByRole("button", { name: /continue/i }));
    await screen.findByRole("heading", { name: /launch at login/i });
    fireEvent.click(screen.getByRole("button", { name: /get started/i }));

    await waitFor(() =>
      expect(
        screen.getByRole("dialog", { name: /^mira controller$/i }),
      ).toBeInTheDocument(),
    );
  });

  it("shows the waiting-for-device heading when nothing is plugged in", async () => {
    const mod = await import("../ipc/__mocks__");
    mod.__setStatus({ connected: false });
    render(
      <Popover
        initialProfiles={sampleProfiles}
        initialStatus={{ connected: false }}
      />,
    );
    await screen.findByRole("dialog", {
      name: /welcome to mira controller/i,
    });
    fireEvent.click(screen.getByRole("button", { name: /continue/i }));
    expect(
      await screen.findByRole("heading", { name: /waiting for a device/i }),
    ).toBeInTheDocument();
  });
});
