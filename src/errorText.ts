import { errorMessage } from "./bridge/errors";
// A path ends where its reason starts (": "), at a quote or at the end.
const WINDOWS_PATH = /[A-Za-z]:[\\/][^\r\n"'`<>]*?(?=:\s|[\r\n"'`<>]|$)/g;
const UNC_PATH = /\\\\[^\\\s"'`<>]+\\[^\r\n"'`<>]*?(?=:\s|[\r\n"'`<>]|$)/g;
const POSIX_PATH = /(?:^|\s)(?:\/[^\s"'`<>:]+)+/g;

/**
 * TXT-02: hides local paths (drive, UNC or POSIX) in an error shown to the
 * person, but keeps the reason that follows the path ("…: acceso denegado").
 */
export function userErrorMessage(error: unknown): string {
  const raw = errorMessage(error);
  const sanitized = raw
    .replace(UNC_PATH, "una ruta local")
    .replace(WINDOWS_PATH, "una ruta local")
    .replace(POSIX_PATH, " una ruta local")
    .trim();
  return (sanitized || "No se pudo completar la operación.").slice(0, 240);
}
