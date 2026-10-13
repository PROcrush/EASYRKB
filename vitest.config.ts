import { defineConfig } from "vitest/config";
import { fileURLToPath, URL } from "node:url";

export default defineConfig({
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  test: {
    environment: "node",
    // `.tsx` for the few tests that mount a component. Those opt into jsdom
    // with a docblock of their own, so everything else stays in node.
    include: ["src/**/*.test.ts", "src/**/*.test.tsx", "design/**/*.test.ts", "scripts/cleanup.test.mjs"],
    reporters: "dot",
    // The shared CI runner can be loaded enough that a component test spends
    // more than the default 5 s importing its modules.
    testTimeout: 20_000,
  },
});
