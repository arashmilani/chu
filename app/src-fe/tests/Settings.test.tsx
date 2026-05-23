import { render, screen, fireEvent } from "@testing-library/react";
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
  launchAtLogin: false,
  hotkeys: {
    profile1: "Ctrl+Alt+1",
    refresh: "Ctrl+Alt+Shift+R",
  },
  autoRefreshEnabled: false,
  autoRefreshSeconds: 30,
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

  it("seeds the chip selection from the currently active profile", async () => {
    render(
      <Settings
        initialAppSettings={baseSettings}
        initialProfiles={sampleProfiles}
        initialActiveProfileId="coding"
      />,
    );
    const codingChip = await screen.findByRole("button", { name: /^Coding$/ });
    const readChip = screen.getByRole("button", { name: /^Read$/ });
    expect(codingChip.getAttribute("data-active")).toBe("true");
    expect(readChip.getAttribute("data-active")).toBe("false");
  });

  it("falls back to the first chip when no active profile is recorded", async () => {
    render(
      <Settings
        initialAppSettings={baseSettings}
        initialProfiles={sampleProfiles}
        initialActiveProfileId={null}
      />,
    );
    const readChip = await screen.findByRole("button", { name: /^Read$/ });
    expect(readChip.getAttribute("data-active")).toBe("true");
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

  it("General tab disables the auto-refresh interval until the toggle is on", async () => {
    render(
      <Settings
        initialAppSettings={baseSettings}
        initialProfiles={sampleProfiles}
      />,
    );
    fireEvent.click(screen.getByRole("tab", { name: "General" }));
    const toggle = await screen.findByRole("checkbox", {
      name: /auto full refresh/i,
    });
    expect((toggle as HTMLInputElement).checked).toBe(false);

    const interval = screen.getByRole("spinbutton", {
      name: /seconds between automatic refreshes/i,
    }) as HTMLInputElement;
    expect(interval.disabled).toBe(true);
    expect(interval.value).toBe("30");

    fireEvent.click(toggle);
    expect(interval.disabled).toBe(false);
  });

  it("General tab commits the auto-refresh interval on blur", async () => {
    render(
      <Settings
        initialAppSettings={{ ...baseSettings, autoRefreshEnabled: true }}
        initialProfiles={sampleProfiles}
      />,
    );
    fireEvent.click(screen.getByRole("tab", { name: "General" }));
    const interval = (await screen.findByRole("spinbutton", {
      name: /seconds between automatic refreshes/i,
    })) as HTMLInputElement;

    fireEvent.change(interval, { target: { value: "45" } });
    fireEvent.blur(interval);
    expect(interval.value).toBe("45");
  });

  it("General tab snaps below-minimum auto-refresh values up to 5 seconds on blur", async () => {
    render(
      <Settings
        initialAppSettings={{ ...baseSettings, autoRefreshEnabled: true }}
        initialProfiles={sampleProfiles}
      />,
    );
    fireEvent.click(screen.getByRole("tab", { name: "General" }));
    const interval = (await screen.findByRole("spinbutton", {
      name: /seconds between automatic refreshes/i,
    })) as HTMLInputElement;

    fireEvent.change(interval, { target: { value: "2" } });
    fireEvent.blur(interval);
    expect(interval.value).toBe("5");
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
