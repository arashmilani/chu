import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

import { Settings } from "../windows/Settings";

vi.mock("../ipc", () => import("../ipc/__mocks__"));

describe("Device tab — multi-device picker", () => {
  beforeEach(async () => {
    const mod = await import("../ipc/__mocks__");
    mod.__setAppSettings({
      applyLastProfileOnConnect: true,
      launchAtLogin: false,
      hotkeys: {},
    });
    mod.__setDevices([
      {
        vendorId: 0x0416,
        productId: 0x5020,
        serialNumber: "abc123",
        productString: "Mira Pro",
      },
      {
        vendorId: 0x0416,
        productId: 0x5020,
        serialNumber: "def456",
        productString: "Mira",
      },
    ]);
  });

  it("lists each connected Mira as a radio with its product string + serial", async () => {
    render(<Settings />);
    await screen.findByRole("checkbox", { name: /launch at login/i });
    fireEvent.click(screen.getByRole("tab", { name: "Device" }));
    expect(await screen.findByText(/Mira Pro/)).toBeInTheDocument();
    expect(screen.getByText(/abc123/)).toBeInTheDocument();
    expect(screen.getByText(/def456/)).toBeInTheDocument();
  });

  it("shows an empty-state message when no devices are present", async () => {
    const mod = await import("../ipc/__mocks__");
    mod.__setDevices([]);
    render(<Settings />);
    await screen.findByRole("checkbox", { name: /launch at login/i });
    fireEvent.click(screen.getByRole("tab", { name: "Device" }));
    expect(
      await screen.findByText(/no mira devices found/i),
    ).toBeInTheDocument();
  });
});
