# Phrase boundaries and unimplemented stages

[Analysis documentation](../README.md) · [Code map](../development.md)

Phrase boundaries are internal outputs of `downbeat.rs`. Phrase labels
and vocal detection are separate interfaces in `phrase.rs` and `vocal.rs`
and remain unimplemented. They do not appear as fields in the public
`Analysis` result.

Read [Downbeat](downbeat.md) for boundary detection. The sections below explain
how boundaries can feed key rules and which outputs must remain absent.

## Phrase starts

Where sections begin: the downbeats at which the music changes most over
the four bars either side. They come out of the downbeat stage
([Downbeat](downbeat.md)) and are used by the key rules that read the
bass at the start of a phrase.

```mermaid
flowchart LR
    A[novelty per half beat<br/>at the 4-bar scale] --> B[local maxima]
    B --> C{on a downbeat,<br/>and at least 30 % of the<br/>strongest peak?}
    C -- yes --> D[phrase start]
    C -- no --> E[ignored]
```

A track has about six of them. They are times in seconds, in order, in
`GridPhase::phrase_starts`, and are passed to key detection as
`KeyGrid::phrase_starts`.

## Phrase labels (not implemented)

Rekordbox's `PSSI` tag labels each section: intro, verse, bridge, chorus,
up, down, outro. There is no open implementation to compare against, and a
wrong label is something a DJ sees mid-set. So `phrase.rs` declares the
shape (`PhraseAnalyzer`, `Phrase`, `PhraseKind`) with an `Unimplemented`
stub that returns `None`, and the ANLZ writer leaves the tag out rather
than write invented structure. The same goes for vocal detection (`PVDI`,
`vocal.rs`).

If labels are built, the phrase starts above are the section boundaries
to label.

## Changing this stage

Follow the [change workflow](../development.md#make-a-change) and run the
[relevant public checks](../validation/README.md#public-tests). Preserve the
input/output contract above and update the reference when options or evidence
change. Report new measurements separately from the recorded results.
