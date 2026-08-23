import fs from "node:fs";

const lockPath = process.argv[2];
if (!lockPath) {
  console.error("Usage: node extract-package-lock-packages.mjs <package-lock.json>");
  process.exit(2);
}

const lock = JSON.parse(fs.readFileSync(lockPath, "utf8"));
const packages = Object.entries(lock.packages ?? {})
  .filter(([path, value]) => path && value && value.version)
  .map(([path, value]) => ({
    path,
    version: String(value.version),
    integrity: value.integrity ?? null,
  }));

process.stdout.write(JSON.stringify(packages));
