import fs from "node:fs";

const lockPath = process.argv[2];
if (!lockPath) {
  console.error("Usage: node extract-package-lock-packages.mjs <package-lock.json>");
  process.exit(2);
}

const lock = JSON.parse(fs.readFileSync(lockPath, "utf8"));
const packageNameFromLockPath = (path) => {
  const marker = "node_modules/";
  const index = path.lastIndexOf(marker);
  return index >= 0 ? path.slice(index + marker.length) : path;
};
const packages = Object.entries(lock.packages ?? {})
  .filter(([path, value]) => path && value && value.version)
  .map(([path, value]) => ({
    name: packageNameFromLockPath(path),
    version: String(value.version),
    license: value.license ?? "UNKNOWN",
    source: value.resolved ?? "package-lock.json",
    integrity: value.integrity ?? null,
  }));

process.stdout.write(JSON.stringify(packages));
