/**
 * My Settings: the player and mixer settings a stick carries in
 * `MYSETTING.DAT`, `MYSETTING2.DAT` and `DJMMYSETTING.DAT`.
 *
 * No capture of this tab was taken, so nothing is drawn: a body guessed at
 * would have to be undone once one is. The tab exists so the strip is the
 * strip rekordbox draws, and it says only what rekordbox itself says for the
 * empty case — "My Settings are not available", the wording `german.lang`
 * gives.
 */
import styles from "./DevicePanel.module.css";

export function MySettingsTab() {
  return (
    <div className={styles.mySettings}>
      <p className={styles.mySettingsNote}>My Settings are not available</p>
    </div>
  );
}
