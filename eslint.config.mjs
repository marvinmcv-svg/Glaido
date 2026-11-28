// Repo-wide ESLint flat config for the JS/TS workspaces (apps/desktop,
// apps/webapp, packages/*). Each package runs `eslint .` from its own folder;
// ESLint walks up to this file. Formatting is Prettier's job, so no stylistic
// rules are enabled here.
import js from "@eslint/js";
import { defineConfig } from "eslint/config";
import nextPlugin from "@next/eslint-plugin-next";
import jsxA11y from "eslint-plugin-jsx-a11y";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

const tsFiles = ["**/*.{ts,tsx,mts,cts}"];

// TypeScript files that no tsconfig includes. They get the syntax-only
// TypeScript rules instead of the type-aware ones.
const untypedTsFiles = [
  "apps/webapp/e2e/**",
  "apps/webapp/playwright.config.ts",
  "packages/ui/vitest.config.ts",
  "packages/ui/.storybook/**",
];

export default defineConfig(
  {
    ignores: [
      "**/node_modules/**",
      "**/dist/**",
      "**/.next/**",
      "**/.turbo/**",
      "**/storybook-static/**",
      "**/test-results/**",
      "**/playwright-report/**",
      "target/**",
      "apps/desktop/src-tauri/**",
      "apps/gateway/**",
      "apps/webapp/next-env.d.ts",
    ],
  },
  {
    linterOptions: { reportUnusedDisableDirectives: "error" },
  },

  js.configs.recommended,

  // Type-aware TypeScript rules, via each package's tsconfig.
  ...tseslint.configs.recommendedTypeChecked.map((config) => ({
    ...config,
    files: tsFiles,
    ignores: untypedTsFiles,
  })),
  {
    files: tsFiles,
    ignores: untypedTsFiles,
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },

  // Syntax-only TypeScript rules for files outside every tsconfig.
  ...tseslint.configs.recommended.map((config) => ({
    ...config,
    files: untypedTsFiles,
  })),

  {
    files: tsFiles,
    rules: {
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_", caughtErrorsIgnorePattern: "^_" },
      ],
    },
  },

  // React: hooks and accessibility, everywhere JSX lives.
  {
    files: ["apps/desktop/src/**/*.tsx", "apps/webapp/**/*.tsx", "packages/ui/src/**/*.tsx"],
    ...reactHooks.configs.flat.recommended,
  },
  {
    files: ["apps/desktop/src/**/*.tsx", "apps/webapp/**/*.tsx", "packages/ui/src/**/*.tsx"],
    ...jsxA11y.flatConfigs.recommended,
  },
  {
    files: ["apps/desktop/src/**/*.{ts,tsx}", "packages/ui/src/**/*.{ts,tsx}"],
    languageOptions: { globals: globals.browser },
  },

  // Next.js rules for the web app.
  {
    files: ["apps/webapp/**/*.{ts,tsx}"],
    plugins: { "@next/next": nextPlugin },
    rules: {
      ...nextPlugin.configs.recommended.rules,
      ...nextPlugin.configs["core-web-vitals"].rules,
    },
    // Absolute, because each package runs ESLint from its own folder.
    settings: { next: { rootDir: `${import.meta.dirname}/apps/webapp/` } },
    languageOptions: { globals: { ...globals.browser, ...globals.node } },
  },

  // Node scripts and tool configs.
  {
    files: ["**/*.{js,mjs,cjs}", "**/*.config.ts", "packages/ui/.storybook/**"],
    languageOptions: { globals: globals.node },
  },
);
