import type { ConfirmDialogOptions, DialogFilter, OpenDialogOptions, SaveDialogOptions } from "@tauri-apps/plugin-dialog";
import { loadCatalog, translate } from "@/i18n";
import { loadPreferences } from "@/lib/preferences";

/** Native dialogs live outside the DOM localizer. Only UI text is translated;
 * default paths, selected paths, extensions and user names are preserved. */
export async function localizedDialog() {
  const [dialog, catalog] = await Promise.all([
    import("@tauri-apps/plugin-dialog"), loadCatalog(loadPreferences().view.locale),
  ]);
  const t = (text: string) => translate(text, catalog);
  const options = <T extends { title?: string; filters?: DialogFilter[] }>(value: T | undefined): T | undefined =>
    value ? {
      ...value,
      ...(value.title !== undefined ? { title: t(value.title) } : {}),
      ...(value.filters !== undefined ? { filters: value.filters.map(filter => ({ ...filter, name: t(filter.name) })) } : {}),
    } : undefined;
  return {
    open: <T extends OpenDialogOptions>(value?: T) => dialog.open(options(value)),
    save: (value?: SaveDialogOptions) => dialog.save(options(value)),
    ask: (text: string, value?: string | ConfirmDialogOptions) => dialog.ask(t(text), typeof value === "string" ? t(value) : {
      ...value,
      ...(value?.title !== undefined ? { title: t(value.title) } : {}),
      okLabel: t(value?.okLabel ?? "OK"), cancelLabel: t(value?.cancelLabel ?? "Cancel"),
    }),
    message: (text: string, value?: Parameters<typeof dialog.message>[1]) => dialog.message(t(text), typeof value === "string" ? t(value) : options(value)),
  };
}
