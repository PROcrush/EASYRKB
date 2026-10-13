# USB export preferences

[Documentation](../README.md) · [USB format reference](../reference/usb-export-db.md)

USB export writes player libraries and audio copies to a device. These
preferences apply to playlist export and Sync Manager; compatibility
conversion also applies to individual track export.

## What a selection exports

- A playlist exports its tracks. An intelligent playlist exports the tracks
  its rule admits at the time of export.
- Each track goes on the stick with the BPM, key, length and analysis files
  your library holds for it when the export starts, including a track
  analysed while an analysis run is still going.
- **Export Folder** exports every playlist and intelligent playlist under the
  folder, at any depth. The device gets the folder, its subfolders (including
  empty ones), and the folders above it, with each playlist inside its folder.
- **Export Playlist** and **Export Folder** (right-click a playlist or folder,
  then choose a connected stick) add to what the stick already holds, as
  rekordbox's do, and so does **Export Track**: playlists and folders exported
  earlier, by either menu or Sync Manager, stay on the stick in their place,
  including empty folders and playlists since deleted from your library, and
  the stick's own playlists are kept. A playlist still in your library is
  updated to what it holds now; one deleted from your library stays as the
  stick has it.
- Sync Manager makes each ticked stick hold exactly the ticked playlists: a
  playlist from your library that is no longer ticked, or that was deleted
  from your library, is taken off the stick.
- Sync Manager lists playlists, intelligent playlists, and folders. Ticking a
  folder ticks every playlist and intelligent playlist under it. The list
  updates when the library changes while the window is open; ticks on
  deleted playlists are removed.

## Playlists on a stick

Open a stick under **Devices** to see the libraries it holds, **Device
Library** and **OneLibrary**, each with **All Tracks** and its **Playlists**,
as rekordbox lists them. Select a playlist to see its tracks.

Right-click the Playlists heading or a folder to create a playlist or folder
on the stick; right-click a playlist or folder to delete it. To rename one,
click it again once it is selected, as in rekordbox. Over the stick's
tracks, **Add To Playlist** lists that library's playlists, and inside a
playlist **Remove from Playlist** takes tracks out. A new playlist goes at
the top, as in rekordbox. When a playlist lists a track twice, selecting
either copy selects both, and Remove from Playlist takes both out (rekordbox
selects and removes each row on its own).

Each change is written to the library it was made in and not the other, as
rekordbox does. Deleting a playlist leaves its tracks on the stick. These
edits are refused while rekordbox is running, and greyed while the stick is
being synced, exported or ejected. Adding tracks from the collection to a
stick's playlist is an export: use Export Playlist or Sync Manager.

## Music already on the device

As in rekordbox, a track whose file the library already keeps on the
destination device is not copied. The device databases name the file where it
is, and export writes only the databases and the track's analysis. A path
with a component the device cannot carry (a reserved character, or a name
ending in a dot or a space) is copied under `Contents/` instead, as rekordbox
does. Export never deletes or replaces such a file, including when the track
later leaves the selection or music cleanup is enabled.

When adding a small playlist to a stick, other device playlists and their
tracks are retained. Their existing audio and analysis files are reused, even
on the first export from this app. Checking may therefore mention the whole
device library while copying is limited to new or changed selected files.

The files to copy must fit in the space currently free. Files scheduled for
cleanup are removed after publication, so their eventual freed space cannot
be used for the copies being staged. The Sync Manager warns about this gap.

## Delete music outside playlists

Off by default. When enabled, export removes RBXport-exported audio outside
all selected playlists, including previously exported individual tracks.
Cleanup runs after the new export is published. Empty playlist selections
are refused; original library audio and unrelated device files are preserved.

## Maximum CDJ compatibility

Before exporting tracks, playlists/folders, or using SYNC, the equipment dialog
also explains audio compatibility and offers two choices for this export:

- **Keep original formats** copies the original encoding. Files unsupported by
  the target player may not play.
- **Convert incompatible copies for older players** converts only files that
  need compatibility processing to WAV, AIFF, or MP3.

The dialog starts with the saved USB Export preference (conversion is off by
default). The choice applies to this export without changing that preference.
Cancelling stops the export. SYNC recalculates its space estimate when this
choice differs from the saved preference.

FLAC and ALAC are unsupported on some older players, including the
[CDJ-2000NXS](https://www.pioneerdj.com/en/product/dj-players-turntables/cdj-2000nxs/).
The [CDJ-2000NXS2 added FLAC/ALAC support](https://www.pioneerdj.com/pl/news/2016/meet-the-new-cdj-2000nxs2-and-djm-900nxs2/),
so choosing traditional Device Library does not by itself mean conversion is
necessary. High-sample-rate or floating-point WAV/AIFF and some AAC/M4A encodings
also depend on the player's model. The warning is general model guidance;
the conversion engine inspects each selected file's encoding when exporting.

When conversion is enabled, incompatible mono/stereo audio is converted to the
selected output format:

| Format | Output |
| --- | --- |
| WAV (default) | 16-bit PCM, 44.1 kHz, stereo; resamples and reduces higher-resolution input. |
| AIFF | 16-bit big-endian PCM, 44.1 kHz, stereo; the same output audio data as WAV after any required resampling and bit-depth reduction. |
| MP3 | 320 kbps CBR, 44.1 kHz, stereo; smaller files with lossy compression. |

Compatible MP3 and integer PCM WAV/AIFF at 44.1 or 48 kHz are copied as-is.
Selecting MP3 does not recompress a compatible WAV or MP3. This targets older
USB-capable players; filesystem and model-specific library requirements
still apply. The [CDJ-350 specifications](https://www.pioneerdj.com/en/product/dj-players-turntables/cdj-350/)
are one format reference, not proof of every player's compatibility.

## Conversion and incremental sync

Conversion changes export copies, preserving source audio and analysis timing.
It updates device database/analysis paths and reuses unchanged conversions on
later syncs. Changing format replaces the previous exported copy.

A track that cannot be converted is omitted and reported; other selected
tracks can still sync. Surround audio is refused instead of dropping channels;
WAV and AIFF output beyond their 32-bit container limits is refused. Structural
verification failures still stop publication.
The app uses Symphonia, Rubato, built-in PCM WAV/AIFF writing, and bundled LAME. Running
export does not require a separate FFmpeg installation.

## Code and tests

`crates/rbl-export/` owns export and publication, working with `rbl-pdb`,
`rbl-onelibrary`, `rbl-anlz`, and device integration. The format reference
explains the full file set and verification boundaries.

The stereo FLAC fixture contains generated 440 Hz left / 880 Hz right tones
at 96 kHz. Tests cover stereo, duration, resampling alignment, MP3 bitrate,
incremental reuse, format changes, source preservation, and failure before
publication. FFmpeg generated that fixture; tests do not require it.
Player behavior needs separate firmware/physical validation; see [Testing](../development/testing.md).


## Choosing playback equipment

Export Track, Export Playlist/Folder, and the Sync Manager ask for the target equipment before writing:

- **Both formats (recommended)** writes Device Library and OneLibrary. Use this when the venue has mixed equipment or the model is unknown. Both databases refer to the same audio copies.
- **CDJ-3000X / OneLibrary equipment** writes OneLibrary for CDJ-3000X, XDJ-AZ, OPUS-QUAD, and OMNIS-DUO.
- **CDJ-3000 and earlier equipment** writes traditional Device Library for CDJ-3000, CDJ-2000NXS2, XDJ-RX3, and similar players.

Existing database formats are also updated, even when another format is selected. This preserves the USB's existing libraries and keeps their track and playlist contents consistent. Cancelling the equipment dialog stops before import or export. The completion summary lists the formats actually updated.

Version `10000` OneLibrary databases are accepted only when all their table definitions match the known export schema. Rebuilding preserves `10000` as well as the existing cue/history/settings protection; an unknown structure remains blocked. A missing database required by the export manifest still fails verification.

Equipment mapping follows the [official CDJ-3000 notice](https://www.pioneerdj.com/en/news/2026/cdj-3000-firmware-ver330-important-notice/) and [OneLibrary FAQ](https://rekordbox.com/en/support/faq/onelibrary-7/), checked on 2026-10-12. Actual CDJ/XDJ playback requires separate hardware acceptance.
