import type { Backend, PinyinReading, PinyinProgress, RowDto } from "./types";

// Only browser fixtures use this small dictionary. The native app uses the
// complete offline Rust dictionary and phrase data.
const WORDS: Record<string, string> = { "七里香": "qi li xiang", "周杰伦": "zhou jie lun", "重庆森林": "chong qing sen lin", "王菲": "wang fei", "女儿情": "nv er qing", "邓丽君": "deng li jun" };
export function createPinyinMock(rows: RowDto[], changed: () => Promise<number>) {
  const overrides = new Map<string, { title: string; artist: string; originalTitle: string; originalArtist: string }>();
  const listeners = new Set<(progress: PinyinProgress) => void>();
  const reading = (row: RowDto): PinyinReading => {
    const custom = overrides.get(row.id);
    const titlePinyin = custom?.originalTitle === row.title && custom.title ? custom.title : WORDS[row.title] ?? "";
    const artistPinyin = custom?.originalArtist === row.artist && custom.artist ? custom.artist : WORDS[row.artist] ?? "";
    const initials = (value: string) => value.split(/\s+/).map(word => word[0] ?? "").join("");
    return { id: row.id, title: row.title, artist: row.artist, titlePinyin, artistPinyin, titleInitials: initials(titlePinyin), artistInitials: initials(artistPinyin) };
  };
  const rowOf = (id: string) => {
    const row = rows.find(row => row.id === id);
    if (!row) throw new Error("Choose a track from the collection.");
    return row;
  };
  const searchValue = (row: RowDto, field: "title" | "artist") => {
    const aliases = reading(row);
    const pinyin = field === "title" ? aliases.titlePinyin : aliases.artistPinyin;
    return `${row[field]} ${pinyin} ${pinyin.replace(/\s/g, "")} ${field === "title" ? aliases.titleInitials : aliases.artistInitials}`;
  };
  const backend: Pick<Backend, "pinyinReading" | "pinyinSave" | "pinyinGenerate" | "onPinyinProgress"> = {
    pinyinReading: id => Promise.resolve(reading(rowOf(id))),
    pinyinSave: async (id, originalTitle, originalArtist, title, artist) => {
      const row = rowOf(id);
      if (row.title !== originalTitle || row.artist !== originalArtist) throw new Error("The track changed. Reopen pronunciation correction.");
      if (!/^[a-zA-Z0-9\sü]*$/.test(title + artist)) throw new Error("Enter pinyin syllables separated by spaces, using v for ü.");
      overrides.set(id, { title: title.trim().toLowerCase().replaceAll("ü", "v"), artist: artist.trim().toLowerCase().replaceAll("ü", "v"), originalTitle, originalArtist });
      await changed();
      return reading(row);
    },
    pinyinGenerate: async (ids, jobId) => {
      const selected = ids ? [...new Set(ids)].map(id => rows.find(row => row.id === id)).filter((row): row is RowDto => !!row) : rows;
      for (const listener of listeners) listener({ jobId, done: 0, total: selected.length });
      await new Promise(resolve => setTimeout(resolve, 20));
      for (const listener of listeners) listener({ jobId, done: selected.length, total: selected.length });
      await changed();
      return { processed: selected.length, chinese: selected.filter(row => /[\u3400-\u9fff]/.test(row.title + row.artist)).length, skipped: (ids ? new Set(ids).size : rows.length) - selected.length };
    },
    onPinyinProgress: listener => { listeners.add(listener); return () => { listeners.delete(listener); }; },
  };
  return { backend, searchValue };
}
