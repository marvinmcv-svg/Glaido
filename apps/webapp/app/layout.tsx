import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: { default: "Vocalis", template: "%s · Vocalis" },
  description: "Speak naturally. Get clean, ready-to-send text in any app.",
  robots: { index: false },
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
