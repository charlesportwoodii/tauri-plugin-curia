import typescript from "@rollup/plugin-typescript";

export default {
  input: "guest-js/index.ts",
  external: [/^@tauri-apps\/api/],
  output: [
    { file: "dist-js/index.js", format: "esm" },
    { file: "dist-js/index.cjs", format: "cjs" },
  ],
  plugins: [
    typescript({
      declaration: true,
      declarationDir: "dist-js",
      rootDir: "guest-js",
    }),
  ],
};
