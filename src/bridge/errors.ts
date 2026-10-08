/**
 * The message Rust returns when the person cancels an operation
 * (`OPERATION_CANCELLED_MESSAGE` in `src-tauri/src/dataset.rs`). A test reads
 * that constant, so changing one side alone fails the suite (COD-01).
 */
export const OPERATION_CANCELLED_MESSAGE = "Operación cancelada por el usuario.";

/** Mirrors LEGACY_ENCODING_PREFIX in Rust: a delimited file that reads as Windows-1252. */
export const LEGACY_ENCODING_PREFIX = "__columnia_legacy_encoding__:windows-1252:";

/** PROD-08: what the interface does differently for an engine error. */
type BridgeErrorCode = "cancelled" | "legacyEncoding" | "other";

/**
 * PROD-08: an engine error, recognised once here by its stable marker
 * (the Rust constants a test compares with these), so the rest of `src`
 * decides by `code` and never by reading the text.
 */
export class BridgeError extends Error {
  readonly code: BridgeErrorCode;
  /** legacyEncoding: the first line of the file decoded as Windows-1252. */
  readonly example: string | null;

  constructor(message: string, code: BridgeErrorCode, example: string | null = null) {
    super(message);
    this.name = "BridgeError";
    this.code = code;
    this.example = example;
  }

  isCancellation(): boolean {
    return this.code === "cancelled";
  }
}

/** The text of anything a promise rejected with. */
export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function toBridgeError(error: unknown): BridgeError {
  if (error instanceof BridgeError) return error;
  const message = errorMessage(error);
  if (message.startsWith(LEGACY_ENCODING_PREFIX)) {
    return new BridgeError(message, "legacyEncoding", message.slice(LEGACY_ENCODING_PREFIX.length));
  }
  if (message.includes(OPERATION_CANCELLED_MESSAGE)) return new BridgeError(message, "cancelled");
  return new BridgeError(message, "other");
}

/** Whether an error from the engine is the person's own cancellation. */
export function isCancellationError(error: unknown): boolean {
  return toBridgeError(error).isCancellation();
}
