import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";
import nextTs from "eslint-config-next/typescript";

const eslintConfig = defineConfig([
  ...nextVitals,
  ...nextTs,
  globalIgnores([
    // Default ignores of eslint-config-next:
    ".next/**",
    "out/**",
    "build/**",
    "next-env.d.ts",
    // Rust build output. Tauri emits generated JS shims under target/, which
    // are not ours to lint.
    "src-tauri/target/**",
    "src-tauri/gen/**",
    // Storybook's static build — bundled third-party code, not ours to lint.
    "storybook-static/**",
    // The usage collector is a Cloudflare Worker, not part of the app: a
    // different runtime, different globals, and its own tsconfig. Linting it
    // with the Next config reports rules that do not apply to it.
    "collector/**",
  ]),
]);

export default eslintConfig;
