import type { NextConfig } from "next";

const config: NextConfig = {
  reactStrictMode: true,
  transpilePackages: ["@vocalis/ui"],
  poweredByHeader: false,
};
export default config;
