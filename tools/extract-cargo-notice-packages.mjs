import { readFileSync } from "node:fs";

const inputPath = process.argv[2];
if (!inputPath) throw new Error("cargo metadata path is required");

const metadata = JSON.parse(readFileSync(inputPath, "utf8"));
const packages = metadata.packages
  .filter((pkg) => pkg.name !== "columnia")
  .map((pkg) => ({
    name: pkg.name,
    version: String(pkg.version),
    license: pkg.license ?? "UNKNOWN",
    source: pkg.source ?? "Cargo.lock",
  }));

process.stdout.write(`${JSON.stringify(packages)}\n`);
