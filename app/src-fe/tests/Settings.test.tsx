import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

import { Settings } from "../windows/Settings";

vi.mock("../ipc", () => import("../ipc/__mocks__"));

describe("Settings", () => {
  beforeEach(async () => {
    const mod = await import("../ipc/__mocks__");
    mod.__setAppSettings({
      applyLastProfileOnConnect: true,
      launchAtLogin: false,
      hotkeys: {
        profile1: "Ctrl+Alt+1",
        refresh: "Ctrl+Alt+Shift+R",
      },
    });
  });

  it("starts on the General tab and reveals the launch-at-login toggle once settings load", async () => {
    render(<Settings />);
    const toggle = await screen.findByRole("checkbox", {
      name: /launch at login/i,
    });
    expect((toggle as HTMLInputElement).checked).toBe(false);
  });

  it("switching to the Hotkeys tab lists the spec §8.1 slots", async () => {
    render(<Settings />);
    await screen.findByRole("checkbox", { name: /launch at login/i });

    fireEvent.click(screen.getByRole("tab", { name: "Hotkeys" }));
    expect(await screen.findByText(/switch to profile 1/i)).toBeInTheDocument();
    expect(screen.getByText(/force full refresh/i)).toBeInTheDocument();
    expect(screen.getByText(/open tray popover/i)).toBeInTheDocument();
  });

  it("renders a Reset hotkeys button on the Hotkeys tab", async () => {
    render(<Settings />);
    await screen.findByRole("checkbox", { name: /launch at login/i });
    fireEvent.click(screen.getByRole("tab", { name: "Hotkeys" }));
    expect(
      await screen.findByRole("button", { name: /reset hotkeys to defaults/i }),
    ).toBeInTheDocument();
  });

  it("Device tab toggles apply-last-on-connect", async () => {
    render(<Settings />);
    await screen.findByRole("checkbox", { name: /launch at login/i });
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
    render(<Settings />);
    await screen.findByRole("checkbox", { name: /launch at login/i });
    fireEvent.click(screen.getByRole("tab", { name: "About" }));
    expect(await screen.findByText(/0\.1\.0-test/i)).toBeInTheDocument();
    expect(screen.getByText("MIT")).toBeInTheDocument();
  });
});
