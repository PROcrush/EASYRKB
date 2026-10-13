# Reference playlist evaluation

[Testing and evaluation](README.md) · [Analysis documentation](../README.md)

This evaluation needs a private reference playlist and audio. Public synthetic checks
run with `RB_LITE_TEST=1 cargo test -p rbl-analysis`. Recorded scores below are
historical measurements.

The recorded full-track evaluation uses the playlist
`RBX-BPM-GRID-TEST` in the installed rekordbox library. 155 tracks, all 4/4,
123–178 BPM, each analysed by rekordbox and its grid checked or set by
hand. The rig is `examples/golden.rs`; it only reads the library.

## Running it

```
cargo run --release -p rbl-analysis --example golden -- cache
cargo run --release -p rbl-analysis --example golden -- eval
cargo run --release -p rbl-analysis --example golden -- eval <title substring>
cargo run --release -p rbl-analysis --example golden -- downbeat
cargo run --release -p rbl-analysis --example golden -- key
cargo run --release -p rbl-analysis --example golden -- bassroot
```

- `cache` decodes every track once and stores the mono PCM with what
  rekordbox recorded — BPM, key and the `PQTZ` beat grid — under
  `target/golden/` (3.6 GB, about two minutes). Delete a track's `.gold`
  file and run `cache` again after re-analysing it in rekordbox.
  `RB_LITE_GOLDEN` points the cache elsewhere, which is how another
  playlist is scored ([Multi-tempo evaluation](multitempo.md)).
- `eval` (or `score`) scores every track in about five seconds. With a
  title substring it scores one track; add `RB_LITE_CANDIDATES=1` to also
  print the tempo candidates, the segments, the fit's passes, the
  half-beat scores and the first beats side by side with rekordbox's.
  `RB_LITE_PLACEMENT=envelope` places beats on the onset envelope's peak
  instead of the kick's attack.
- `downbeat` checks the downbeat stage on rekordbox's own grids, so it is
  judged apart from our grid.
- `key` measures every front end, profile and rule — what each fires on,
  fixes and breaks — and searches the bass-rule knobs. `bassroot` measures
  the bass as evidence by itself, per band and window.

## What is scored

| metric | passes when |
|---|---|
| bpm | within 0.05 BPM of rekordbox's, no octave folding |
| downbeat | the offset between our downbeats and rekordbox's, taken modulo a bar, is within 25 ms |
| grid | at least 98 % of rekordbox's beats have one of ours within 25 ms carrying the same beat number |
| key | the same name rekordbox chose |

The grid metric is the strict one: a tempo right to 0.05 BPM can still
drift a beat off by the end of the track, and a tempo change has to be
placed at the right beat for the numbering after it to match.

## Recorded results before transient emphasis

These playlist measurements and miss descriptions predate the transition
fallback that boosts transient rises by 4× with a 20 ms release. They
have not been remeasured for that change. The synthetic regression
coverage is documented in
the private fixture guide (`rbxport-private/crates/rbl-analysis/docs/grid-fixtures.md`, “Transitions without kicks”).

| metric | passes |
|---|---|
| bpm | 155 / 155 |
| downbeat | 143 / 155 |
| grid | 142 / 155 |
| key | 141 / 155 |

The target is 99 %, at most one miss per metric. Every BPM is a whole
number, as every one of rekordbox's is. Beats are placed on the kick's
attack: the offset from rekordbox 7's grids is 0.0 ms at the median and at
the 90th percentile. On the 142 tracks rekordbox 7 analysed itself: BPM
142 / 142, downbeat 141 / 142, grid 142 / 142. Analysis takes about 460 ms
per track at the mean.

## The misses

**Eleven tracks analysed by rekordbox 6.** *See Me Now, technikore &
weaver – diya, Missing, Insane Stampede, Sky Fall, Concrete Jungle, Voltage,
Goddess, Drift Around Me, Angels, Force of Gravity.* The playlist was
analysed at different times, some of it by rekordbox 6 and the rest by 7.
These eleven grids sit exactly 1105 samples (25 ms) after the kick; the
other 140 MP3s sit on it. Our beats are on the kick, so all eleven fail
both the downbeat and the grid metric, at −25 to −27 ms, just past the
tolerance. The offset is not in the file or the decoder: it is not the
Xing/Info frame, the LAME tag's CRC, the encoder version, the ID3 version,
or anything in `djmdContent`, and rekordbox's own bundled `libmpg123`
gives exactly our timeline (gapless off) or exactly 1105 samples earlier
(gapless on), never later. Re-analysing them in rekordbox 7 would settle
it.

**One DJ edit, gridded by hand.** `Go Back [136-174]`: 136 BPM until bar
61, rising until bar 82.2, then 174. The hand grid holds 136 through the
rise, nudged half a beat somewhere in it, and switches at bar 82.25. The
rise begins in a breakdown with no kick to track. In the recorded run the
walker did not follow it: the grid held 136 straight and cut where the kick stated 174 —
2 ms from the hand grid, numbered as it numbers — and matches 89 % of its
beats, all but the nudged bars of the rise. Its beat 1 on the 136 section
is one beat before rekordbox's, which is the downbeat miss: the novelty
peaks land a beat before each phrase change. (`Bring Me Back to Life
[138-150]`, the other DJ edit, passes: its 150 kick pattern plays at half
level under the 138 breakdown from 76 s, stops for two bars, and drops at
105.158 s, where the cut goes.)

**BATTERY OPERATED.** Two 130 BPM grids: one from 0.051 s to 146.359 s
and another from 147.029 s, 208 ms (0.45 beat) later than the first would
put it. Between them, from bar 65 (118.2 s), the kick stops and an
eighth-note bass slows under a filter to an eighth of 1.4 s at 2:22, then
silence, then 130 again at 2:27.0. The gap stage ([Beat grid](../algorithms/beat.md), §8)
fits the halves on their own, cuts at 147.029 s, and walks the slowdown
beat by beat (25 beats, 130 → 22 BPM). Downbeat passes at −2 ms and the
second half matches the hand grid in time and number. The grid metric
fails at 91 % because the hand grid holds 130 through the slowdown (62
beats) where ours follows the bass (25 beats). Re-gridding the original
with the slowdown would settle it.

**Key.** 14 misses, listed in [Key detection](../algorithms/key.md).

## What rekordbox does

Facts about rekordbox's grids established while building the gate:

- The grid is on the untrimmed MP3 timeline. A track whose first kick is
  at sample 0 of the master has its first beat at ~24 ms — the LAME encoder
  delay plus decoder delay, 1105 samples. Symphonia with
  `enable_gapless: false` matches it.
- Rekordbox 7 bundles `libmpg123.0.dylib` in `rekordbox.app/Contents/MacOS`.
  Its output is 32-bit float.
- Grids are constant-tempo to within 1–3 ms over a whole track. A tempo
  change is a second constant grid with the 1–4 count carrying on and a
  new phase.
- The grid starts at the first grid position after time zero, extended
  back from the music, even over silence or a beatless intro.
- Ten of the 155 tracks have beat 1 one to three beats after the first
  beat.
- `.EXT` files can carry an empty `PQT2`; the `.DAT`'s `PQTZ` is the grid.
