# Waveform overview calibration

[Documentation](../README.md) · [Analysis crate](../../crates/rbl-analysis/README.md)

RBX computes `PWV6` separately from the 150 Hz detail waveform. Peak-reducing
that detail produced the regular cream spikes and shallow blue band seen in
the Cannonball comparison. Changing the drawing height alone cannot fix it.

The 1,200 overview buckets now use:

- Bass: 200 Hz one-pole low-pass RMS, raised to 0.9 for mild compression.
- Mids: cascaded second-order Butterworth high-pass at 200 Hz and low-pass
  at 2 kHz; mean absolute amplitude.
- Highs: second-order Butterworth high-pass at 2 kHz; mean absolute amplitude.
- Each band's mean is normalized to 92 times the track's mean 150 Hz-window
  RMS. A two-bucket trailing average supplies the envelope's short release.
- Rounded seven-bit values are written directly, without the old 44-value
  ceiling. Detail and other waveform formats retain their existing algorithms.

These are empirical approximations, not recovered rekordbox DSP. Calibration
used the nine tracks in `RBX-BPM-MULTIBPM-TEST`; those tracks are not a held-out
accuracy test. The remaining mismatch is principally track-level gain.

## Recorded calibration results

At 1,200 columns and 40px height, compare all three stacked band boundaries
using the renderer's scales (128, 256, 128). Mean absolute error in pixels:

| Track | Previous | Current |
| --- | ---: | ---: |
| Cannonball | 3.65 | 0.60 |
| Bring Me Back to Life | 2.62 | 0.65 |
| Castles — Festival | 2.48 | 0.70 |
| Around the World | 2.49 | 0.68 |
| It Feels So Good | 2.93 | 0.86 |
| Castles — EDCLV | 2.10 | 1.11 |
| Sao Paulo | 2.69 | 2.00 |
| Battery Operated | 2.91 | 1.96 |
| Go Back | 1.74 | 2.18 |

Cannonball's outer envelope alone averages 0.95px error. Go Back regresses in
absolute height even though its shape correlation improves; universal
pixel-perfect parity is not established. Pixel error scales with display height.

An internal calibration harness compares saved results against originals,
checking overview band boundaries as well as browser-height and shape checks.
The 2.5px boundary gate is a regression limit for those fixtures, not a claim
of pixel-perfect output. The captured tracks and comparison tooling are kept
outside this public repository.

## Read-only diagnostics

For audio diagnostics that write separate output:

```sh
cargo run --release -p rbl-analysis --example waveform_probe -- AUDIO OUTPUT_DIRECTORY
```

This writes decoded mono samples, sample rate, and generated PWV6/PWV7 payloads.
## Manual repair tool

This operation writes analysis files in the selected library. It is not an
onboarding or automated test command. Use only a disposable development
library, with rekordbox closed. To refresh only the overview:

```sh
cargo run --release -p rbl-analysis --example repair_waveforms -- RBX-BPM-MULTIBPM-RESULTS --overview-only
```

The repair backs up changed analysis files under `verification/preview-repair`
and preserves every section except PWV6, including grids, cues and detail
waveforms. Reopen the track/app to discard already-loaded waveform data.
