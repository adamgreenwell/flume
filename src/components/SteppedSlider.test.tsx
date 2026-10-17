import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { PEER_STEPS, SteppedSlider } from "./SteppedSlider";

function slider(props: Partial<Parameters<typeof SteppedSlider>[0]> = {}) {
  const onChange = vi.fn();
  render(
    <SteppedSlider
      steps={PEER_STEPS}
      value={128}
      label="Connect to at most"
      format={(n) => `${n} peers`}
      onChange={onChange}
      width="w-[86px]"
      {...props}
    />,
  );
  return { onChange, input: screen.getByLabelText("Connect to at most") };
}

/** Index of a value in PEER_STEPS, for driving the range input. */
function stop(value: number) {
  const index = PEER_STEPS.indexOf(value);
  expect(index).toBeGreaterThanOrEqual(0);
  return String(index);
}

describe("committing on release", () => {
  it("reports once for a drag across many stops, not once per stop", () => {
    // The reason this mode exists. The peer limit rebuilds the librqbit
    // session, so a drag that reported every stop it crossed would queue an
    // engine restart for each one -- tearing down the listener and
    // re-announcing every torrent, a dozen times, for one gesture.
    const { onChange, input } = slider({ commit: "release" });

    for (const value of [160, 200, 300, 500]) {
      fireEvent.change(input, { target: { value: stop(value) } });
    }
    expect(onChange).not.toHaveBeenCalled();

    fireEvent.pointerUp(input);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(500);
  });

  it("tracks the caret while the drag is uncommitted", () => {
    // Deferring the commit must not defer the feedback: a slider whose number
    // only moves when you let go reads as broken.
    const { input } = slider({ commit: "release" });

    fireEvent.change(input, { target: { value: stop(300) } });

    expect(screen.getByText("300 peers")).toBeDefined();
    expect(input.getAttribute("aria-valuetext")).toBe("300 peers");
  });

  it("commits a keyboard adjustment on key release", () => {
    const { onChange, input } = slider({ commit: "release" });

    fireEvent.change(input, { target: { value: stop(160) } });
    fireEvent.keyUp(input, { key: "ArrowRight" });

    expect(onChange).toHaveBeenCalledExactlyOnceWith(160);
  });

  it("commits on blur, for a drag released off the control", () => {
    const { onChange, input } = slider({ commit: "release" });

    fireEvent.change(input, { target: { value: stop(80) } });
    fireEvent.blur(input);

    expect(onChange).toHaveBeenCalledExactlyOnceWith(80);
  });

  it("stays quiet when a release follows no change", () => {
    const { onChange, input } = slider({ commit: "release" });

    fireEvent.pointerUp(input);
    fireEvent.blur(input);

    expect(onChange).not.toHaveBeenCalled();
  });
});

describe("committing on change", () => {
  it("reports every stop, which is what a live setting wants", () => {
    // Unchanged behaviour for the rate, ratio, duration and count sliders:
    // those apply without a restart, and a figure that only moves on release
    // would feel unresponsive for no gain.
    const { onChange, input } = slider();

    fireEvent.change(input, { target: { value: stop(160) } });
    fireEvent.change(input, { target: { value: stop(200) } });

    expect(onChange).toHaveBeenCalledTimes(2);
    expect(onChange).toHaveBeenNthCalledWith(1, 160);
    expect(onChange).toHaveBeenNthCalledWith(2, 200);
  });
});
