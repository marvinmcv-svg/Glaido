import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { Button, DictationPill, Keycap, keyLabel, StatCard, Switch } from "./index";

describe("keyLabel", () => {
  it("maps per platform", () => {
    expect(keyLabel("cmd", "macos")).toBe("⌘");
    expect(keyLabel("cmd", "windows")).toBe("Win");
    expect(keyLabel("cmd", "linux")).toBe("Super");
    expect(keyLabel("right-option", "macos")).toBe("Right ⌥");
    expect(keyLabel("left ctrl", "windows")).toBe("Left Ctrl");
    expect(keyLabel("k", "macos")).toBe("K");
  });
});

describe("Keycap", () => {
  it("renders caps with a spoken label", () => {
    render(<Keycap keys={["cmd", "k"]} platform="macos" />);
    expect(screen.getByRole("img", { name: "Command plus K" })).toBeInTheDocument();
    expect(screen.getAllByText(/⌘|K/)).toHaveLength(2);
  });
});

describe("Switch", () => {
  function Harness() {
    const [on, setOn] = useState(false);
    return <Switch checked={on} onChange={setOn} label="Interaction sounds" />;
  }
  it("toggles aria-checked", () => {
    render(<Harness />);
    const sw = screen.getByRole("switch", { name: "Interaction sounds" });
    expect(sw).toHaveAttribute("aria-checked", "false");
    fireEvent.click(sw);
    expect(sw).toHaveAttribute("aria-checked", "true");
  });
  it("respects disabled", () => {
    const onChange = vi.fn();
    render(<Switch checked={false} onChange={onChange} label="x" disabled />);
    fireEvent.click(screen.getByRole("switch"));
    expect(onChange).not.toHaveBeenCalled();
  });
});

describe("Button", () => {
  it("defaults to type=button and applies the variant", () => {
    render(<Button variant="attention">Go</Button>);
    const b = screen.getByRole("button", { name: "Go" });
    expect(b).toHaveAttribute("type", "button");
    expect(b.className).toContain("vc-button--attention");
  });
});

describe("StatCard", () => {
  it("shows label, value and hint", () => {
    render(<StatCard label="Time saved" value="12h 30m" hint="About a day and a half" />);
    expect(screen.getByText("12h 30m")).toBeInTheDocument();
    expect(screen.getByText("About a day and a half")).toBeInTheDocument();
  });
});

describe("DictationPill", () => {
  it("announces state and clamps levels", () => {
    const { container } = render(<DictationPill state="recording" levels={[0, 0.5, 2, -1]} />);
    expect(screen.getByRole("status")).toHaveTextContent("Listening");
    const heights = [...container.querySelectorAll<HTMLElement>(".vc-pill__bar")].map(
      (b) => b.style.height,
    );
    expect(heights).toEqual(["4px", "14px", "24px", "4px"]);
  });
});
