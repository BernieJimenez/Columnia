import { createHash, generateKeyPairSync, sign } from "node:crypto";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const generatorPath = join(projectRoot, "tools", "generate-updater-manifest.mjs");
const checkerPath = join(projectRoot, "tools", "check-updater-manifest.mjs");
const keyPolicyPath = join(projectRoot, "tools", "check-updater-key-policy.mjs");

function fail(message) {
  throw new Error(message);
}

function runNode(scriptPath, args, env = { COLUMNIA_UPDATER_CONTRACT_TEST: "1" }) {
  const result = spawnSync(process.execPath, [scriptPath, ...args], {
    cwd: projectRoot,
    encoding: "utf8",
    windowsHide: true,
    env: { ...process.env, ...env },
  });
  if (result.error) fail(`No se pudo ejecutar ${scriptPath}: ${result.error.message}`);
  return result;
}

function expectSuccess(result, label) {
  if (result.status !== 0) {
    fail(`${label} debía aprobar y terminó con ${result.status}: ${(result.stderr || result.stdout).trim()}`);
  }
}

function expectFailure(result, label) {
  if (result.status === 0) fail(`${label} debía fallar, pero aprobó.`);
}

function writeJson(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function createEphemeralMinisignKey() {
  const { publicKey, privateKey } = generateKeyPairSync("ed25519");
  const publicKeyDer = publicKey.export({ format: "der", type: "spki" });
  const keyId = Buffer.from("columT5!", "ascii");
  const publicKeyBytes = Buffer.concat([Buffer.from("Ed"), keyId, publicKeyDer.subarray(-32)]);
  const publicKeyText = [
    "untrusted comment: minisign public key: Columnia contract fixture",
    publicKeyBytes.toString("base64"),
    "",
  ].join("\n");
  return {
    encodedPublicKey: Buffer.from(publicKeyText, "utf8").toString("base64"),
    signArtifact(artifact) {
      const primary = Buffer.concat([
        Buffer.from("ED"),
        keyId,
        sign(null, createHash("blake2b512").update(artifact).digest(), privateKey),
      ]);
      const trustedComment = "trusted comment: timestamp: 2026-08-28T00:00:00Z";
      const trustedSignature = sign(
        null,
        Buffer.concat([primary.subarray(10), Buffer.from(trustedComment.slice("trusted comment: ".length), "utf8")]),
        privateKey,
      );
      return Buffer.from([
        "untrusted comment: signature from Columnia contract fixture",
        primary.toString("base64"),
        trustedComment,
        trustedSignature.toString("base64"),
        "",
      ].join("\n"), "utf8").toString("base64");
    },
  };
}

const fixtureRoot = mkdtempSync(join(tmpdir(), "columnia-updater-contract-"));
const bundleRoot = join(fixtureRoot, "bundle");
const artifactPath = join(bundleRoot, "nsis", "Columnia_0.57.0_x64-setup.exe");
const signaturePath = `${artifactPath}.sig`;
const manifestPath = join(fixtureRoot, "latest.json");
const inventoryPath = join(fixtureRoot, "inventory.json");
const signingFixture = createEphemeralMinisignKey();
const checkerArgs = (overrides = {}) => [
  "--manifest", manifestPath,
  "--inventory", inventoryPath,
  "--public-key", signingFixture.encodedPublicKey,
  "--expected-version", overrides.version ?? "0.57.0",
  "--allowed-host", overrides.host ?? "updates.example.invalid",
];

try {
  mkdirSync(dirname(artifactPath), { recursive: true });
  writeFileSync(artifactPath, Buffer.from("Columnia updater contract fixture\n", "utf8"));
  writeFileSync(signaturePath, signingFixture.signArtifact(readFileSync(artifactPath)), "utf8");

  const generatorArgs = [
    "--bundle-root", bundleRoot,
    "--output", manifestPath,
    "--inventory-output", inventoryPath,
    "--base-url", "https://updates.example.invalid/columnia/",
    "--target", "windows-x86_64",
    "--version", "0.57.0",
    "--artifact", artifactPath,
  ];
  expectSuccess(runNode(generatorPath, generatorArgs), "Generación del fixture");
  // COD-19: pub_date is the artifact's build time, not the generation time.
  const generated = JSON.parse(readFileSync(manifestPath, "utf8"));
  if (generated.pub_date !== statSync(artifactPath).mtime.toISOString()) {
    fail(`pub_date ${generated.pub_date} no es la fecha del artefacto.`);
  }
  if (process.platform === "win32") {
    // COD-19: Windows paths differ only in case still point inside the bundle.
    const upperArgs = [...generatorArgs];
    upperArgs[upperArgs.indexOf("--artifact") + 1] = artifactPath.toUpperCase();
    expectSuccess(runNode(generatorPath, upperArgs), "Artefacto con otras mayúsculas");
    expectSuccess(runNode(generatorPath, generatorArgs), "Regeneración del fixture");
  }
  const outsideArgs = [...generatorArgs];
  outsideArgs[outsideArgs.indexOf("--artifact") + 1] = join(fixtureRoot, "bundle-other", "x.exe");
  expectFailure(runNode(generatorPath, outsideArgs), "Artefacto fuera del bundle");
  expectSuccess(
    runNode(checkerPath, checkerArgs()),
    "Contrato válido",
  );

  // SEG-04: a manifest for another version (a downgrade), a foreign host or
  // a substitute public key outside the contract test do not pass.
  expectFailure(runNode(checkerPath, checkerArgs({ version: "0.58.0" })), "Versión distinta de la publicada");
  expectFailure(runNode(checkerPath, checkerArgs({ host: "cdn.example.invalid" })), "Host ajeno");
  expectFailure(runNode(checkerPath, checkerArgs(), {}), "Clave pública sustituida fuera de la prueba");

  const originalArtifact = readFileSync(artifactPath);
  writeFileSync(artifactPath, originalArtifact.subarray(0, Math.max(1, originalArtifact.length - 1)));
  expectFailure(
    runNode(checkerPath, checkerArgs()),
    "Artefacto truncado",
  );
  writeFileSync(artifactPath, originalArtifact);

  const originalSignature = readFileSync(signaturePath, "utf8");
  const originalInventory = readFileSync(inventoryPath, "utf8");
  const originalManifest = readFileSync(manifestPath, "utf8");
  const cryptographicallyAlteredSignature = Buffer.from(originalSignature, "base64");
  const cryptographicallyAlteredText = cryptographicallyAlteredSignature.toString("utf8").split(/\r?\n/);
  const alteredPrimary = Buffer.from(cryptographicallyAlteredText[1], "base64");
  alteredPrimary[20] ^= 0x01;
  cryptographicallyAlteredText[1] = alteredPrimary.toString("base64");
  const cryptographicallyAltered = Buffer.from(cryptographicallyAlteredText.join("\n"), "utf8").toString("base64");
  writeFileSync(signaturePath, cryptographicallyAltered, "utf8");
  const alteredManifest = JSON.parse(originalManifest);
  alteredManifest.platforms["windows-x86_64"].signature = cryptographicallyAltered;
  writeJson(manifestPath, alteredManifest);
  const alteredInventory = JSON.parse(originalInventory);
  alteredInventory.artifact.signatureSha256 = sha256(signaturePath);
  alteredInventory.manifest.sha256 = sha256(manifestPath);
  writeJson(inventoryPath, alteredInventory);
  expectFailure(
    runNode(checkerPath, checkerArgs()),
    "Firma criptográficamente inválida",
  );
  writeFileSync(manifestPath, originalManifest, "utf8");
  writeFileSync(inventoryPath, originalInventory, "utf8");

  // SEG-11: an edited trusted comment fails even with a valid primary signature.
  const commentLines = Buffer.from(originalSignature, "base64").toString("utf8").split(/\r?\n/);
  commentLines[2] = "trusted comment: timestamp: 2099-01-01T00:00:00Z";
  const alteredComment = Buffer.from(commentLines.join("\n"), "utf8").toString("base64");
  writeFileSync(signaturePath, alteredComment, "utf8");
  const commentManifest = JSON.parse(originalManifest);
  commentManifest.platforms["windows-x86_64"].signature = alteredComment;
  writeJson(manifestPath, commentManifest);
  const commentInventory = JSON.parse(originalInventory);
  commentInventory.artifact.signatureSha256 = sha256(signaturePath);
  commentInventory.manifest.sha256 = sha256(manifestPath);
  writeJson(inventoryPath, commentInventory);
  expectFailure(runNode(checkerPath, checkerArgs()), "Comentario de confianza alterado");
  writeFileSync(manifestPath, originalManifest, "utf8");
  writeFileSync(inventoryPath, originalInventory, "utf8");

  // SEG-11: a policy missing a required text field does not pass.
  const keyPolicy = JSON.parse(readFileSync(join(projectRoot, "fixtures", "updater", "key-policy-v1.json"), "utf8"));
  const keyPolicyCopy = join(fixtureRoot, "key-policy.json");
  expectSuccess(runNode(keyPolicyPath, []), "Política de claves versionada");
  for (const [section, field] of [["rotation", "oldPrivateKeyRetirement"], ["recovery", "manualRecovery"]]) {
    const altered = structuredClone(keyPolicy);
    delete altered[section][field];
    writeJson(keyPolicyCopy, altered);
    expectFailure(runNode(keyPolicyPath, ["--policy", keyPolicyCopy]), `Política sin ${field}`);
  }

  writeFileSync(
    signaturePath,
    Buffer.from("untrusted comment: altered signature fixture\n", "utf8").toString("base64"),
    "utf8",
  );
  expectFailure(
    runNode(checkerPath, checkerArgs()),
    "Firma alterada",
  );
  writeFileSync(signaturePath, originalSignature, "utf8");

  const incompleteManifest = JSON.parse(originalManifest);
  delete incompleteManifest.platforms;
  writeJson(manifestPath, incompleteManifest);
  expectFailure(
    runNode(checkerPath, checkerArgs()),
    "Manifiesto incompleto",
  );

  writeFileSync(manifestPath, "{\n", "utf8");
  expectFailure(
    runNode(checkerPath, checkerArgs()),
    "Manifiesto JSON corrupto",
  );
  writeFileSync(manifestPath, originalManifest, "utf8");

  const insecureGeneratorArgs = [...generatorArgs];
  insecureGeneratorArgs[insecureGeneratorArgs.indexOf("--base-url") + 1] = "http://updates.example.invalid/columnia/";
  expectFailure(runNode(generatorPath, insecureGeneratorArgs), "URL updater insegura");

  const inventory = JSON.parse(readFileSync(inventoryPath, "utf8"));
  if (inventory.artifact.sha256 !== sha256(artifactPath)) {
    fail("El fixture generado no conserva el hash del artefacto.");
  }

  console.log("Contrato updater aprobado: válido, versión distinta, host ajeno, clave sustituida, truncado, firma o comentario de confianza alterados, política de claves incompleta, manifiesto incompleto/corrupto y URL insegura.");
} finally {
  rmSync(fixtureRoot, { recursive: true, force: true });
}
