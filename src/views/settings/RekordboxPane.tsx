import { useId } from "react";
import { usePreferencesContext } from "@/store/usePreferences";
import { Section } from "./controls";
import layout from "./PaneLayout.module.css";
import controls from "./Preferences.module.css";
import option from "./UsbExportPane.module.css";

export function RekordboxPane() {
  const { preferences, update } = usePreferencesContext();
  const id = useId();
  return (
    <Section title="Browse settings">
      <p className={`${layout.help} ${option.intro}`}>Choose how EASYRKB uses rekordbox browse settings.</p>
      <label className={`${layout.summary} ${option.option}`}>
        <span className={option.details}>
          <strong id={`${id}-sync`}>Keep browse settings synchronized</strong>
          <span id={`${id}-sync-help`} className={option.description}>
            Import supported column visibility, order, and widths, plus browser pane widths, from rekordbox when EASYRKB starts.
          </span>
          <span className={option.default}>Default: On</span>
        </span>
        <input type="checkbox" role="switch" className={controls.toggle}
          aria-labelledby={`${id}-sync`} aria-describedby={`${id}-sync-help`}
          checked={preferences.rekordbox.syncBrowseSettings}
          onChange={(event) => update("rekordbox", { syncBrowseSettings: event.target.checked })} />
      </label>
    </Section>
  );
}
