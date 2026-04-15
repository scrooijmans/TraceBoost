import { access } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(scriptDir, "..");

const requiredPaths = [
  {
    label: "@traceboost/seis-contracts",
    path: path.resolve(frontendRoot, "../../contracts/ts/seis-contracts/package.json"),
    fix: "Generate the contracts package if it is missing."
  },
  {
    label: "@ophiolite/charts",
    path: path.resolve(frontendRoot, "../../../ophiolite/charts/packages/svelte/package.json"),
    fix: "Clone the sibling ophiolite repository at ../ophiolite if it is missing."
  },
  {
    label: "@ophiolite/charts-data-models",
    path: path.resolve(frontendRoot, "../../../ophiolite/charts/packages/data-models/package.json"),
    fix: "Clone the sibling ophiolite repository at ../ophiolite if it is missing."
  },
  {
    label: "@ophiolite/charts-core",
    path: path.resolve(frontendRoot, "../../../ophiolite/charts/packages/chart-core/package.json"),
    fix: "Clone the sibling ophiolite repository at ../ophiolite if it is missing."
  },
  {
    label: "@ophiolite/charts-domain",
    path: path.resolve(frontendRoot, "../../../ophiolite/charts/packages/domain-geoscience/package.json"),
    fix: "Clone the sibling ophiolite repository at ../ophiolite if it is missing."
  },
  {
    label: "@ophiolite/charts-renderer",
    path: path.resolve(frontendRoot, "../../../ophiolite/charts/packages/renderer/package.json"),
    fix: "Clone the sibling ophiolite repository at ../ophiolite if it is missing."
  },
  {
    label: "@ophiolite/contracts",
    path: path.resolve(frontendRoot, "../../../ophiolite/contracts/ts/ophiolite-contracts/package.json"),
    fix: "Clone the sibling ophiolite repository at ../ophiolite and generate its contracts package if it is missing."
  }
];

const missing = [];

for (const entry of requiredPaths) {
  try {
    await access(entry.path);
  } catch {
    missing.push(entry);
  }
}

if (missing.length > 0) {
  console.error("Missing local development dependencies for traceboost-frontend:");
  for (const entry of missing) {
    console.error(`- ${entry.label}: ${entry.path}`);
    console.error(`  ${entry.fix}`);
  }
  process.exit(1);
}

console.log("Local development dependencies are present.");
