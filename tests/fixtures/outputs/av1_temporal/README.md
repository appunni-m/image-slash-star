# Animated AVIF temporal candidate evidence

This is independent implementation evidence for `AVF-SEQUENCE-001`, collected
from the existing complete `animated.avif`. It is not an active public parity
row or a coverage measurement. The Rust regression is compile-checked but has
not been executed; behavioral testing remains deferred by the task instruction.

## Regenerate

Use a clean dav1d checkout at
`b546257f770768b2c88258c533da38b91a06f737` and the locked Pillow environment:

```sh
CC=/usr/bin/clang .oracle-venv/bin/python scripts/generate_av1_temporal_refs.py \
  --dav1d-source /path/to/pinned/dav1d \
  --meson /opt/homebrew/bin/meson --ninja /opt/homebrew/bin/ninja
```

The default destination is ignored `target/oracle-staging/av1-temporal/animated`.
An existing destination is never replaced; `--output` can select a fresh
directory for a later independent run. Native compiler and binary identities
are retained with each result. Debug binary hashes can vary with build paths;
compare event records and raw displayed YUV separately from build metadata.

## Observations and causal boundary

The wrapper renames `add_temporal_candidate`, records its initialized inputs,
calls the unchanged original body exactly once, and records its outputs.
Invalid entries have no denominator; single-reference candidates have no
second vector. These absent fields are JSON null, never uninitialized reads.
The complete instrumentation diff, source commit/tree/license hashes, input
hash, build commands, compiler identities, and artifact hashes are in the
bundle. dav1d's license is retained in `third_party/dav1d/COPYING`.

There are 177 observed lookups. Of these, 145 contain invalid entries and do
not modify state. The 32 valid single-reference candidates produce two
insertions, 30 weight increases, and two global-context changes from 1 to 0.
For example, event 16 has references `[1, -1]`, source vector `[0, 0]`,
denominator 1, and an empty stack; dav1d inserts `[0, 0]` with weight 2.
Event 17 raises that candidate's weight to 4.

The former Rust loop attempted both reference slots and returned at the `-1`
sentinel, before either stack insertion or context update. The repair projects
one vector for a single reference and two for a compound reference.

The unmodified decode and both instrumented runs have exactly equal displayed
YUV (168,750 bytes; SHA-256
`62884b195d2489cab53a1763749b73c57219d773ddbeb569aa5f92c394a5bcd0`).
The two traces are also byte-identical. Pinned Pillow independently observes
five RGB frames, 150×150 pixels each; its frame hashes and timing observations
are retained in the index.

## Regression and limits

`tests/support/av1_temporal.rs` consumes every event and compares all 32 valid
candidate transitions, including complete ordered stacks and context. The
coverage-only adapter normalizes the observed signed distances into equivalent
seven-bit order hints, and maps the absent second vector to Rust's zero slot.
These are representation conversions; expected results come only from dav1d.
The adapter reads no fixture data and calls the same private production helper.

No compound, nonzero-vector, or lower-precision candidate is observed in this
fixture. This evidence does not establish general projection, frame-header
parsing, reference-field loading, final public sequence pixels, or the complete
AVIF animation contract. Those obligations remain open. No status is promoted.
