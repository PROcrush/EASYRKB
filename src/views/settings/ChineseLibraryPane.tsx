import { useTranslation } from "@/i18n";
import { PinyinGenerator } from "@/views/library/PinyinTools";
import { Section } from "./controls";
import styles from "./ChineseLibraryPane.module.css";

export function ChineseLibraryPane() {
  const t = useTranslation();
  return <div className={styles.pane}>
    <Section title={t("Chinese library search")}>
      <div className={styles.content}>
        <div className={styles.paragraphs}>
          <p>{t("Search Chinese titles and artists with Chinese text, full pinyin or initials. Combine artist and title, for example: zjl qlx.")}</p>
          <p>{t("Pinyin is indexed automatically when tracks are loaded or renamed. Regeneration preserves your manual pronunciation corrections.")}</p>
        </div>
        <div className={styles.generator}><PinyinGenerator /></div>
        <div className={`${styles.paragraphs} ${styles.help}`}>
          <p>{t("Right-click a collection track to generate pinyin for the selection or correct its pronunciation.")}</p>
          <p>{t("Pinyin search keeps the original Chinese titles and artists. It does not change audio tags or rekordbox metadata.")}</p>
          <p>{t("Pronunciation corrections are stored locally in EASYRKB for this library.")}</p>
        </div>
      </div>
    </Section>
  </div>;
}
