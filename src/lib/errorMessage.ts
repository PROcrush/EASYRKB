/** Tauri rejects commands with plain { kind, message, detail } objects. */
export function errorMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string") {
    const detail = "detail" in error && typeof error.detail === "string" ? error.detail.trim() : "";
    return detail && detail !== error.message ? `${error.message} ${detail}` : error.message;
  }
  if (typeof error === "string") return error;
  return "Something went wrong. Please try again.";
}
