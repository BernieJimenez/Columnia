import { createHash, createPublicKey, verify } from "node:crypto";

function strictBase64(value, label) {
  if (typeof value !== "string" || !/^[A-Za-z0-9+/]+={0,2}$/.test(value) || value.length % 4 !== 0) {
    throw new Error(`${label} no es Base64 estricto.`);
  }
  return Buffer.from(value, "base64");
}

function parsePublicKey(encodedPublicKey) {
  const publicKeyText = strictBase64(encodedPublicKey, "La clave pública updater").toString("utf8");
  const publicKeyLines = publicKeyText.split(/\r?\n/).filter(Boolean);
  const publicKeyLine = publicKeyLines.find((line) => /^[A-Za-z0-9+/]+={0,2}$/.test(line));
  if (!publicKeyLine) throw new Error("La clave pública updater no contiene un bloque minisign.");
  const publicKey = strictBase64(publicKeyLine, "El bloque de clave pública updater");
  if (publicKey.length !== 42 || publicKey.subarray(0, 2).toString("ascii") !== "Ed") {
    throw new Error("La clave pública updater no usa el formato Ed25519 esperado.");
  }
  const publicKeyDer = Buffer.concat([
    Buffer.from("302a300506032b6570032100", "hex"),
    publicKey.subarray(10),
  ]);
  return {
    bytes: publicKey,
    keyObject: createPublicKey({ key: publicKeyDer, format: "der", type: "spki" }),
  };
}

export function verifyMinisign(artifact, encodedPublicKey, encodedSignature) {
  const publicKey = parsePublicKey(encodedPublicKey);
  const signatureText = strictBase64(encodedSignature, "La firma updater").toString("utf8");
  const signatureLines = signatureText.split(/\r?\n/).filter(Boolean);
  if (!signatureLines[0]?.startsWith("untrusted comment:") || !signatureLines[2]?.startsWith("trusted comment:")) {
    throw new Error("La firma updater no contiene los comentarios minisign esperados.");
  }
  const signature = strictBase64(signatureLines[1], "La firma primaria updater");
  const trustedSignature = strictBase64(signatureLines[3], "La firma del comentario updater");
  if (signature.length !== 74 || trustedSignature.length !== 64 || signature.subarray(0, 2).toString("ascii") !== "ED") {
    throw new Error("La firma updater no usa el formato Ed25519 esperado.");
  }
  if (!signature.subarray(2, 10).equals(publicKey.bytes.subarray(2, 10))) {
    throw new Error("La firma updater pertenece a una clave distinta de la clave pública embebida.");
  }

  const digest = createHash("blake2b512").update(artifact).digest();
  if (!verify(null, digest, publicKey.keyObject, signature.subarray(10))) {
    throw new Error("La firma minisign no valida el contenido descargado.");
  }
  return {
    algorithm: "Ed25519 over BLAKE2b-512",
    fingerprint: publicKey.bytes.subarray(2, 10).toString("hex").toUpperCase(),
    trustedCommentPresent: true,
    verified: true,
  };
}
