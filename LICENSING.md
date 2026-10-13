# Licensing

rbxport is licensed as a whole under GPL-2.0-or-later. See
[`LICENSE`](LICENSE); the complete GPL text is included in
[`crates/rbl-deck/vendor/rubberband/COPYING`](crates/rbl-deck/vendor/rubberband/COPYING).

## Rubber Band

`crates/rbl-deck/vendor/rubberband` is the [Rubber Band
Library](https://breakfastquay.com/rubberband/) v4.0.0, vendored, and it is
GPL-2.0-or-later or a commercial licence from Particular Programs Ltd. This
project takes the GPL. Its own copyright and terms are in
`crates/rbl-deck/vendor/rubberband/COPYING`, unmodified.

Linking it makes the binary a combined work, so anything distributed from a
default build must comply with the GPL, including the corresponding-source
requirements.

It is here because a DJ deck needs a stretcher that holds a pitch. The WSOLA
backend written for this crate stretches tempo well and misses an interval by
25 to 50 cents, which is a key shift that puts a track in the wrong key.
Rubber Band R3 lands the same intervals inside 1.4 cents.

## Building without it

```sh
cargo build -p rbl-deck --no-default-features
```

The `rubberband` feature is on by default. With it off, `MASTER TEMPO` falls
back to the WSOLA backend and key shifting is not offered.

## Offline phrase pinyin dictionary

`crates/rbl-index/data/phrases.tsv` derives from mozillazg/phrase-pinyin-data
version 0.19.0 (MIT). Its attribution and complete license are retained in
`crates/rbl-index/data/phrase-pinyin-LICENSE` and included in desktop bundles.
See the adjacent README for the pinned source and transformation.
