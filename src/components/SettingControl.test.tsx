import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { SettingControl } from "./SettingControl";

function textControl(value: string | null = null) {
  const onChange = vi.fn();
  render(
    <SettingControl
      control={{ kind: "text", placeholder: "https://…" }}
      value={value}
      label="Refuse peers in this list"
      onChange={onChange}
    />,
  );
  return {
    onChange,
    input: screen.getByLabelText("Refuse peers in this list"),
  };
}

describe("a text setting", () => {
  it("reports nothing while the user is still typing", () => {
    // The reason this holds a draft. Every setting writes straight through to
    // `update_settings`, which validates — and the first character of
    // `https://…` is `h`, which is not a URL. Reporting per keystroke meant
    // the write was rejected and the field restored before the second
    // character arrived, so the only way to set one was to paste it whole.
    const { onChange, input } = textControl();

    for (const value of ["h", "ht", "htt", "http", "https://lists.example"]) {
      fireEvent.change(input, { target: { value } });
    }

    expect(onChange).not.toHaveBeenCalled();
    expect((input as HTMLInputElement).value).toBe("https://lists.example");
  });

  it("commits the finished value on blur", () => {
    const { onChange, input } = textControl();

    fireEvent.change(input, { target: { value: "https://lists.example" } });
    fireEvent.blur(input);

    expect(onChange).toHaveBeenCalledExactlyOnceWith("https://lists.example");
  });

  it("commits on Enter, for a user who does not tab away", () => {
    const { onChange, input } = textControl();

    fireEvent.change(input, { target: { value: "https://lists.example" } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onChange).toHaveBeenCalledExactlyOnceWith("https://lists.example");
  });

  it("keeps Escape from reaching the dialog that would close on it", () => {
    // The settings dialog closes on Escape from a listener on `window`, so
    // without stopping propagation, abandoning a draft also dismissed the
    // whole screen — throwing away the rest of it to undo one field.
    const onWindowEscape = vi.fn();
    window.addEventListener("keydown", onWindowEscape);
    try {
      const { input } = textControl("https://old.example");

      fireEvent.change(input, { target: { value: "nonsense" } });
      fireEvent.keyDown(input, { key: "Escape" });

      expect(onWindowEscape).not.toHaveBeenCalled();
    } finally {
      window.removeEventListener("keydown", onWindowEscape);
    }
  });

  it("abandons the draft on Escape", () => {
    // The only way out once you have typed something you did not mean to
    // save, and the usual contract for an editable field.
    const { onChange, input } = textControl("https://old.example");

    fireEvent.change(input, { target: { value: "nonsense" } });
    fireEvent.keyDown(input, { key: "Escape" });

    expect((input as HTMLInputElement).value).toBe("https://old.example");
    fireEvent.blur(input);
    expect(onChange).not.toHaveBeenCalled();
  });

  it("reports an emptied field as unset rather than as an empty string", () => {
    const { onChange, input } = textControl("https://old.example");

    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.blur(input);

    expect(onChange).toHaveBeenCalledExactlyOnceWith(null);
  });

  it("stays quiet when the value is retyped unchanged", () => {
    const { onChange, input } = textControl("https://lists.example");

    fireEvent.change(input, { target: { value: "https://lists.example" } });
    fireEvent.blur(input);

    expect(onChange).not.toHaveBeenCalled();
  });
});
