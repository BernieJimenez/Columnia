import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const IMAGE_SUBSYSTEM_WINDOWS_GUI = 2;
const SUBSYSTEM_NAMES = new Map([
  [2, "WINDOWS_GUI"],
  [3, "WINDOWS_CUI (consola)"],
]);

/** Reads the Subsystem field from a PE32/PE32+ image. */
export function readPeSubsystem(bytes) {
  if (bytes.length < 0x40 || bytes[0] !== 0x4d || bytes[1] !== 0x5a) {
    throw new Error("El archivo no es un ejecutable PE (falta la firma MZ).");
  }
  const peOffset = bytes.readUInt32LE(0x3c);
  if (bytes.length < peOffset + 24 + 70 || bytes.readUInt32LE(peOffset) !== 0x00004550) {
    throw new Error("El archivo no contiene una cabecera PE válida.");
  }
  const optionalHeader = peOffset + 24;
  const magic = bytes.readUInt16LE(optionalHeader);
  if (magic !== 0x10b && magic !== 0x20b) {
    throw new Error(`Cabecera opcional PE desconocida: 0x${magic.toString(16)}.`);
  }
  // Subsystem sits at the same offset (68) in PE32 and PE32+ optional headers.
  return bytes.readUInt16LE(optionalHeader + 68);
}

export function checkGuiSubsystem(executablePath) {
  const subsystem = readPeSubsystem(readFileSync(executablePath));
  return {
    subsystem,
    name: SUBSYSTEM_NAMES.get(subsystem) ?? `desconocido (${subsystem})`,
    passed: subsystem === IMAGE_SUBSYSTEM_WINDOWS_GUI,
  };
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : "";
if (import.meta.url === invokedPath) {
  const executable = resolve(process.argv[2] ?? "src-tauri/target/release/columnia.exe");
  const result = checkGuiSubsystem(executable);
  if (!result.passed) {
    console.error(`El ejecutable release usa el subsistema ${result.name}; debe ser WINDOWS_GUI. Revisa windows_subsystem en src-tauri/src/main.rs.`);
    process.exitCode = 1;
  } else {
    console.log(`Subsistema PE aprobado: ${result.name}.`);
  }
}
