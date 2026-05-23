import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

import { Settings } from "../windows/Settings";

vi.mock("../ipc", () => import("../ipc/__mocks__"));

const sampleProfiles = [
  {
    id: "read",
    name: "Read",
    builtIn: true,
    hotkey: null,
    settings: {
      refreshMode: "direct" as const,
      speed: 3,
      contrast: 9,
      ditherMode: 1,
      whiteFilter: 0,
      blackFilter: 0,
      coldLight: 0,
      warmLight: 0,
    },
    createdAt: "1970-01-01T00:00:00Z",
    modifiedAt: "1970-01-01T00:00:00Z",
  },
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

const baseSettings = {
  applyLastProfileOnConnect: true,
  launchAtLogin: false,
  hotkeys: {
    profile1: "Ctrl+Alt+1",
    refresh: "Ctrl+Alt+Shift+R",
  },
};

describe("Settings", () => {
  beforeEach(async () => {
    const mod = await import("../ipc/__mocks__");
    mod.__setAppSettings(baseSettings);
    mod.__setProfiles(sampleProfiles);
  });

  it("opens on the Profiles tab and lists profile chips", async () => {
    render(
      <Settings
        initialAppSettings={baseSettings}
        initialProfiles={sampleProfiles}
      />,
    );
    expect(
      await screen.findByRole("button", { name: /^Read$/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /^Coding$/ }),
    ).toBeInTheDocument();
  });

  it("Profiles tab shows Duplicate + Reset on a built-in selection", async () => {
    render(
      <Settings
        initialAppSettings={baseSettings}
        initialProfiles={sampleProfiles}
      />,
    );
    expect(
      await screen.findByRole("button", { name: /^duplicate$/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /reset to defaults/i }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /^delete$/i }),
    ).not.toBeInTheDocument();
  });

  it("Profiles tab shows Duplicate + Delete on a custom selection", async () => {
    const customProfiles = [
      {
        ...sampleProfiles[0],
        id: "550e8400-e29b-41d4-a716-446655440000",
        name: "My Custom",
        builtIn: false,
      },
    ];
    render(
      <Settings
        initialAppSettings={baseSettings}
        initialProfiles={customProfiles}
      />,
    );
    expect(
      await screen.findByRole("button", { name: /^duplicate$/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /^delete$/i }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /reset to defaults/i }),
    ).not.toBeInTheDocument();
  });

  it("General tab toggles launch-at-login", async () => {
    render(
      <Settings
        initialAppSettings={baseSettings}
        initialProfiles={sampleProfiles}
      />,
    );
    fireEvent.click(screen.getByRole("tab", { name: "General" }));
    const toggle = await screen.findByRole("checkbox", {
      name: /launch at login/i,
    });
    expect((toggle as HTMLInputElement).checked).toBe(false);
  });

  it("Hotkeys tab lists the slot labels (no openPopover anymore)", async () => {
    render(
      <Settings
        initialAppSettings={baseSettings}
        initialProfiles={sampleProfiles}
      />,
    );
    fireEvent.click(screen.getByRole("tab", { name: "Hotkeys" }));
    expect(await screen.findByText(/switch to profile 1/i)).toBeInTheDocument();
    expect(screen.getByText(/^refresh$/i)).toBeInTheDocument();
    expect(screen.queryByText(/open tray popover/i)).not.toBeInTheDocument();
  });

  it("Device tab toggles apply-last-on-connect", async () => {
    render(
      <Settings
        initialAppSettings={baseSettings}
        initialProfiles={sampleProfiles}
      />,
    );
    fireEvent.click(screen.getByRole("tab", { name: "Device" }));
    const toggle = await screen.findByRole("checkbox", {
      name: /re-apply last profile/i,
    });
    expect((toggle as HTMLInputElement).checked).toBe(true);
    fireEvent.click(toggle);
    await waitFor(() =>
      expect((toggle as HTMLInputElement).checked).toBe(false),
    );
  });

  it("About tab shows version and license", async () => {
    render(
      <Settings
        initialAppSettings={baseSettings}
        initialProfiles={sampleProfiles}
        initialVersion="9.9.9-test"
      />,
    );
    fireEvent.click(screen.getByRole("tab", { name: "About" }));
    expect(await screen.findByText(/9\.9\.9-test/)).toBeInTheDocument();
    expect(screen.getByText("MIT")).toBeInTheDocument();
  });
});
