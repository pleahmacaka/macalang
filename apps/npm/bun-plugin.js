// Bun plugin: import `.maca` files directly.
//
//   // bunfig.toml
//   preload = ["macalang/bun"]
//
//   // then anywhere:
//   import { add } from "./example.maca";
//
// The plugin compiles each `.maca` to ESM JavaScript on load.

import { plugin } from "bun";
import { readFileSync } from "node:fs";
import { toESM } from "./index.js";

plugin({
  name: "macalang",
  setup(build) {
    build.onLoad({ filter: /\.maca$/ }, (args) => {
      const src = readFileSync(args.path, "utf8");
      return { contents: toESM(src), loader: "js" };
    });
  },
});
