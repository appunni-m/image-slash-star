# Security policy

## Supported versions

Until the first stable release, security fixes are applied to the latest
commit on `main`.

## Reporting a vulnerability

Please use
[GitHub private vulnerability reporting](https://github.com/appunni-m/image-slash-star/security/advisories/new)
for this repository. Do not open a public issue containing exploit details,
malicious image samples, or information that would put users at risk. Include
the affected format, smallest reproducer, impact, and any suggested mitigation.

Maintainers will acknowledge a report as soon as practical, coordinate a fix
and disclosure with the reporter, and credit reporters who wish to be named.

The pre-release API does not yet expose caller-controlled decode limits. Exact
fixture parity, complete coverage, and strict static checks must not be treated
as a claim that arbitrary hostile input is safe.
