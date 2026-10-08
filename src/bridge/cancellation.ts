// PROD-08: the cancellation marker and its check live with the other bridge
// errors; this module keeps the import path the controllers already use.
export { isCancellationError, OPERATION_CANCELLED_MESSAGE } from "./errors";
