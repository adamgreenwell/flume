import type { Metadata, Viewport } from "next";

import { DevMockGate } from "@/components/DevMockGate";

import "./globals.css";

export const metadata: Metadata = {
  title: "Flume",
  description: "A beautiful, cross-platform BitTorrent client.",
};

export const viewport: Viewport = {
  themeColor: "#0b0e14",
  // The app lives in a fixed-size desktop window; pinch-zoom would only ever
  // be an accident here.
  initialScale: 1,
  width: "device-width",
  maximumScale: 1,
};

/**
 * Root layout for the Flume application shell.
 *
 * @param props - Standard Next.js layout props.
 * @returns The HTML document shell.
 */
export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" className="h-full">
      <body className="bg-bg-0 text-fg-0 min-h-full antialiased">
        <DevMockGate>{children}</DevMockGate>
      </body>
    </html>
  );
}
