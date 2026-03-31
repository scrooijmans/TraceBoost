import { spawnSync } from "node:child_process";
import path from "node:path";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig, type Plugin } from "vite";

const host = process.env.TAURI_DEV_HOST;

function traceboostDevApi(): Plugin {
  const repoRoot = path.resolve(__dirname, "../..");

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

  async function readJsonBody(req: NodeJS.ReadableStream & { setEncoding(encoding: BufferEncoding): void }): Promise<Record<string, string>> {
    return await new Promise((resolve, reject) => {
      let body = "";
      req.setEncoding("utf8");
      req.on("data", (chunk) => {
        body += chunk;
      });
      req.on("end", () => {
        try {
          resolve(body ? JSON.parse(body) : {});
        } catch (error) {
          reject(error);
        }
      });
      req.on("error", reject);
    });
  }

  return {
    name: "traceboost-dev-api",
    configureServer(server) {
      server.middlewares.use("/api/preflight", async (req, res) => {
        try {
          const body = await readJsonBody(req);
          const inputPath = body.inputPath?.trim();
          if (!inputPath) {
            res.statusCode = 400;
            res.end("Missing inputPath");
            return;
          }
          const payload = runCargo([
            "run",
            "-q",
            "-p",
            "traceboost-app",
            "--",
            "preflight-import",
            inputPath
          ]);
          res.setHeader("Content-Type", "application/json");
          res.end(payload);
        } catch (error) {
          res.statusCode = 500;
          res.end(error instanceof Error ? error.message : "Unknown preflight error");
        }
      });

      server.middlewares.use("/api/import", async (req, res) => {
        try {
          const body = await readJsonBody(req);
          const inputPath = body.inputPath?.trim();
          const outputStorePath = body.outputStorePath?.trim();
          if (!inputPath || !outputStorePath) {
            res.statusCode = 400;
            res.end("Missing inputPath or outputStorePath");
            return;
          }
          const payload = runCargo([
            "run",
            "-q",
            "-p",
            "traceboost-app",
            "--",
            "import-dataset",
            inputPath,
            outputStorePath
          ]);
          res.setHeader("Content-Type", "application/json");
          res.end(payload);
        } catch (error) {
          res.statusCode = 500;
          res.end(error instanceof Error ? error.message : "Unknown import error");
        }
      });

      server.middlewares.use("/api/open", async (req, res) => {
        try {
          const body = await readJsonBody(req);
          const storePath = body.storePath?.trim();
          if (!storePath) {
            res.statusCode = 400;
            res.end("Missing storePath");
            return;
          }
          const payload = runCargo([
            "run",
            "-q",
            "-p",
            "traceboost-app",
            "--",
            "open-dataset",
            storePath
          ]);
          res.setHeader("Content-Type", "application/json");
          res.end(payload);
        } catch (error) {
          res.statusCode = 500;
          res.end(error instanceof Error ? error.message : "Unknown open-store error");
        }
      });

      server.middlewares.use("/api/section", (req, res) => {
        try {
          const url = new URL(req.url ?? "/", "http://localhost");
          const storePath = url.searchParams.get("storePath")?.trim();
          const axis = url.searchParams.get("axis") ?? "inline";
          const index = url.searchParams.get("index") ?? "0";
          if (!storePath) {
            res.statusCode = 400;
            res.end("Missing storePath");
            return;
          }
          const body = runCargo([
            "run",
            "-q",
            "-p",
            "traceboost-app",
            "--",
            "view-section",
            storePath,
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
  plugins: [svelte(), traceboostDevApi()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"]
    }
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: process.env.TAURI_ENV_DEBUG ? false : "esbuild",
    sourcemap: Boolean(process.env.TAURI_ENV_DEBUG)
  },
  resolve: {
    alias: {
      "@geoviz/data-models": path.resolve(__dirname, "../../../geoviz/packages/data-models/src/index.ts"),
      "@geoviz/chart-core": path.resolve(__dirname, "../../../geoviz/packages/chart-core/src/index.ts"),
      "@geoviz/renderer": path.resolve(__dirname, "../../../geoviz/packages/renderer/src/index.ts"),
      "@geoviz/domain-geoscience": path.resolve(
        __dirname,
        "../../../geoviz/packages/domain-geoscience/src/index.ts"
      ),
      "@geoviz/svelte": path.resolve(__dirname, "../../../geoviz/packages/svelte/src/index.ts"),
      "@traceboost/seis-contracts": path.resolve(
        __dirname,
        "../../contracts/ts/seis-contracts/src/index.ts"
      )
    }
  }
});
