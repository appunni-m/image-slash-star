# Support

`image-slash-star` is pre-release software. The canonical behavior and
pending-work status live in [docs/roadmap-new.md](docs/roadmap-new.md); the
historical roadmap is background only.

Use [GitHub issues](https://github.com/appunni-m/image-slash-star/issues) for
questions, reproducible bugs, and feature requests that do not contain
sensitive material. A useful report includes:

- the crate revision or release version;
- target triple, operating system, Rust version, enabled Cargo features, and
  whether the `avif` feature is enabled and which planned AVIF class is involved;
- the smallest non-sensitive input or a way to regenerate it;
- the public API call and policy/options used; and
- the expected result, actual result, and the first failing verification
  command.

Do not attach malicious samples or exploit details to a public issue. Use the
private process in [SECURITY.md](SECURITY.md) instead. Conduct concerns belong
to the process in [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

Support is best effort until the first stable release. A passing Pillow parity
row or coverage result is evidence for that bounded contract; it is not a
promise that every legal image or hostile-input scenario is supported.
