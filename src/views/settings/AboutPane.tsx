import { useEffect, useState } from "react";
import { getBackend } from "@/ipc/client";
import styles from "./Preferences.module.css";

/** Version and licence information for the locally maintained Chinese build. */
export function AboutPane() {
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    void getBackend().then(backend => backend.appVersion()).then(found => {
      if (live) setVersion(found);
    }).catch(() => {});
    return () => { live = false; };
  }, []);

  return <>
    <section className={styles.section} aria-label="About">
      <div className={styles.aboutHeader}>
        <img className={styles.aboutLogo} src="/branding/easyrkb-logo.png" alt="" />
        <div>
          <h1 className={styles.aboutName}>EASYRKB</h1>
          <p className={styles.aboutVersion} data-testid="about-version">v{version ?? "—"}</p>
        </div>
      </div>
      <p className={styles.aboutLine}>This build uses local reports and manual updates.</p>
    </section>
    <div className={styles.aboutLegal}>
      <details>
        <summary>Licence</summary>
        <p className={styles.aboutLine}>
          EASYRKB is free software, licensed under the GNU General Public License version 2 or, at your
          option, any later version. It comes with no warranty. It includes the Rubber Band Library
          by Particular Programs Ltd., under the same licence.
        </p>
      </details>
      <details>
        <summary>Disclaimer</summary>
        <p className={styles.aboutLine}>
          EASYRKB is an independent project and is not affiliated with, endorsed by, or sponsored by
          AlphaTheta Corporation, Pioneer DJ, or any other third party. rekordbox, CDJ, XDJ and
          PRO DJ LINK are trademarks of their respective owners. Back up your library before using
          any feature that writes to it.
        </p>
      </details>
    </div>
  </>;
}
