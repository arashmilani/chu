import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";

import { SettingsForm } from "../components/SettingsForm";

const sample = {
  refreshMode: "a2" as const,
  speed: 6,
  contrast: 12,
  ditherMode: 0,
  whiteFilter: 16,
  blackFilter: 8,
  coldLight: 0,
  warmLight: 0,
};

describe("SettingsForm", () => {
  it("renders nine settings (refresh mode + 7 sliders) with current values", () => {
    render(<SettingsForm initial={sample} />);
    // Refresh-mode radios are present.
    expect(screen.getByRole("radio", { name: /A2/ })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /Direct/ })).toBeInTheDocument();
    // Seven sliders for the numeric settings.
    expect(screen.getAllByRole("slider")).toHaveLength(7);
  });

  it("range inputs reflect spec bounds", () => {
    render(<SettingsForm initial={sample} />);
    const speed = screen.getByRole("slider", {
      name: /refresh speed/i,
    }) as HTMLInputElement;
    expect(speed.min).toBe("1");
    expect(speed.max).toBe("7");

    const white = screen.getByRole("slider", {
      name: /whiten background/i,
    }) as HTMLInputElement;
    expect(white.min).toBe("0");
    expect(white.max).toBe("127");

    const cold = screen.getByRole("slider", {
      name: /cool front light/i,
    }) as HTMLInputElement;
    expect(cold.max).toBe("254");
  });

  it("emits onChange with the updated value when a slider moves", () => {
    const handler = vi.fn();
    render(<SettingsForm initial={sample} onChange={handler} />);

    const contrast = screen.getByRole("slider", {
      name: /contrast/i,
    }) as HTMLInputElement;
    fireEvent.change(contrast, { target: { value: "10" } });
    expect(handler).toHaveBeenCalledWith(
      expect.objectContaining({ contrast: 10 }),
    );
  });

  it("renders all controls disabled when the disabled prop is set", () => {
    render(<SettingsForm initial={sample} disabled />);
    for (const slider of screen.getAllByRole("slider")) {
      expect((slider as HTMLInputElement).disabled).toBe(true);
    }
  });

  it("clicking the Direct radio switches refreshMode", () => {
    const handler = vi.fn();
    render(<SettingsForm initial={sample} onChange={handler} />);
    const direct = screen.getByRole("radio", { name: /Direct/ });
    fireEvent.click(direct);
    expect(handler).toHaveBeenCalledWith(
      expect.objectContaining({ refreshMode: "direct" }),
    );
  });
});
