import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import path from "node:path";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig, type Plugin } from "vite";

function traceboostSectionApi(): Plugin {
  const repoRoot = path.resolve(__dirname, "../..");
  const storeRoot = path.resolve(__dirname, ".cache/demo-store.zarr");
  const fixturePath = path.resolve(repoRoot, "test-data/small.sgy");

  function runCargo(args: string[]): string {
    const result = spawnSync("cargo", args, {
      cwd: repoRoot,
      encoding: "utf8"
    });
    if (result.status !== 0) {
      throw new Error(result.stderr || result.stdout || "cargo command failed");
    }
    return result.stdout.trim();
  }

  function ensureDemoStore(): void {
    if (existsSync(storeRoot)) {
      return;
    }
    mkdirSync(path.dirname(storeRoot), { recursive: true });
    runCargo([
      "run",
      "-q",
      "-p",
      "traceboost-app",
      "--",
      "ingest",
      fixturePath,
      storeRoot
    ]);
  }

  return {
    name: "traceboost-section-api",
    configureServer(server) {
      server.middlewares.use("/api/section", (req, res) => {
        try {
          ensureDemoStore();
          const url = new URL(req.url ?? "/", "http://localhost");
          const axis = url.searchParams.get("axis") ?? "inline";
          const index = url.searchParams.get("index") ?? "0";
          const body = runCargo([
            "run",
            "-q",
            "-p",
            "traceboost-app",
            "--",
            "view-section",
            "--store",
            storeRoot,
            "--axis",
            axis,
            index
          ]);
          res.setHeader("Content-Type", "application/json");
          res.end(body);
        } catch (error) {
          res.statusCode = 500;
          res.setHeader("Content-Type", "application/json");
          res.end(
            JSON.stringify({
              message: error instanceof Error ? error.message : "Unknown backend bridge error"
            })
          );
        }
      });
    }
  };
}

export default defineConfig({
  plugins: [svelte(), traceboostSectionApi()]
});
