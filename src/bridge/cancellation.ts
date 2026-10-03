/**
 * The message Rust returns when the person cancels an operation
 * (`OPERATION_CANCELLED_MESSAGE` in `src-tauri/src/dataset.rs`). A test reads
 * that constant, so changing one side alone fails the suite (COD-01).
 */
export const OPERATION_CANCELLED_MESSAGE = "Operación cancelada por el usuario.";

/** Whether an error from the engine is the person's own cancellation. */
export function isCancellationError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return message.includes(OPERATION_CANCELLED_MESSAGE);
}
