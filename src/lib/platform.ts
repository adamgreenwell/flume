import type { WindowControls } from "@/components/TitleBar";

/**
 * Which side the OS draws its window buttons on.
 *
 * Read from the user-agent rather than through `@tauri-apps/plugin-os`: this is
 * a layout inset, needed on first paint, and adding a plugin and a capability
 * to learn something the webview already knows synchronously would cost a
 * round trip and a permission for no gain.
 *
 * Anything that is not recognisably a Mac gets the right-hand inset. macOS
 * user-agents always contain "Macintosh", so an unrecognised string is far
 * more likely to be a Linux build than a Mac — and being wrong the other way
 * puts an 88px hole in the left of the title bar on every Linux machine.
 *
 * @param userAgent - The UA string to inspect.
 * @returns Which side to reserve space on.
 */
export function windowControlsFor(userAgent: string): WindowControls {
  // Matches "Macintosh" and "Mac OS X"; iOS is irrelevant here since Flume is
  // a desktop app, and matching it would be harmless anyway.
  return /Mac|iPhone|iPad/i.test(userAgent) ? "left" : "right";
}

/**
 * Subscribes to changes in where the window buttons are.
 *
 * There are none — the platform does not change under a running app. The
 * callback is accepted and ignored so this can be read with
 * `useSyncExternalStore`, which is the only way to get a value that legitimately
 * differs between the static export and the client without either a hydration
 * mismatch or a `setState` in an effect.
 *
 * @returns An unsubscribe function that does nothing.
 */
export function subscribeToWindowControls(): () => void {
  return () => {};
}

/**
 * Which side the current environment draws its window buttons on.
 *
 * @returns Which side to reserve space on.
 */
export function detectWindowControls(): WindowControls {
  if (typeof navigator === "undefined") return "left";
  return windowControlsFor(navigator.userAgent);
}

/**
 * What the static export renders before the client knows the platform.
 *
 * macOS, because that is where Flume is developed and the inset it needs is the
 * smaller of the two — a briefly-too-small reserved area is less visible than a
 * briefly-too-large one.
 *
 * @returns The build-time default.
 */
export function serverWindowControls(): WindowControls {
  return "left";
}
