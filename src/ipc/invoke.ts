import type { InvokeArgs } from "@tauri-apps/api/core";
import { errorMessage } from "@/lib/errorMessage";
import type { AppErrorDto } from "./types";

/**
 * A command failure, as an `Error`. The backend rejects with a plain
 * `{ kind, message, detail }` object, which prints as nothing useful and
 * which `instanceof Error` checks pass over; this keeps its message (detail
 * included) and its `kind`.
 */
export class IpcError extends Error {
  readonly kind: AppErrorDto["kind"] | undefined;
  readonly detail: string | undefined;

  constructor(source: unknown) {
    super(errorMessage(source), { cause: source });
    this.name = "IpcError";
    const fields = source && typeof source === "object" ? source as Partial<AppErrorDto> : {};
    this.kind = fields.kind;
    this.detail = fields.detail;
  }
}

/** Tauri's `invoke`, rejecting with an `IpcError` rather than a bare object. */
export async function tauriInvoke(): Promise<<T>(command: string, args?: InvokeArgs) => Promise<T>> {
  const { invoke } = await import("@tauri-apps/api/core");
  return async <T>(command: string, args?: InvokeArgs) => {
    try {
      return await invoke<T>(command, args);
    } catch (error) {
      throw error instanceof Error ? error : new IpcError(error);
    }
  };
}
