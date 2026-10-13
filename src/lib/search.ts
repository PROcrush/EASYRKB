export const TRACK_SEARCH_OPTIONS = [
  { value: "all", label: "All" }, { value: "title", label: "Track Title" },
  { value: "artist", label: "Artist" }, { value: "album", label: "Album" },
  { value: "genre", label: "Genre" }, { value: "year", label: "Year" },
  { value: "bpm", label: "BPM" }, { value: "composer", label: "Composer" },
  { value: "albumArtist", label: "Album Artist" }, { value: "remixer", label: "Remixer" },
  { value: "label", label: "Label" }, { value: "comment", label: "Comments" },
  { value: "originalArtist", label: "Original Artist" }, { value: "mixName", label: "Mix Name" },
] as const;
export type TrackSearchField = typeof TRACK_SEARCH_OPTIONS[number]["value"];
export const TREE_SEARCH_OPTIONS = [
  { value: "all", label: "All" }, { value: "playlist", label: "Playlist" },
  { value: "folder", label: "Folder" },
] as const;
