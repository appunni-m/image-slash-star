# Documentation maintenance

The site is generated from this repository's public guides and validated evidence.
`documentation.json` selects source pages; `mkdocs.yml` owns navigation.
`target/site/` is reproducible output and remains untracked.

## Review checklist

- [ ] Start with the audience's installation or integration task.
- [ ] Verify package names, exact versions, ownership, errors, and examples.
- [ ] Separate declared capabilities, executed parity, source coverage, and performance.
- [ ] Keep partial, planned, excluded, pending, and unmeasured paths visible.
- [ ] Bind every reported measurement to its source, inputs, runner, and date.
- [ ] Preserve legal notices and the final Puhu/Pillow acknowledgements.
- [ ] Regenerate contracts and inventories through their Make targets.
- [ ] Remove superseded internal plans after preserving durable public guidance.
- [ ] Check local links, rendered HTML, keyboard use, narrow layouts, and search.

## Build and preview

```sh
make docs-setup
make docs-test docs-lint
make docs-examples
make docs-build
make docs-serve
```

Documentation uses Python 3.12.10 in CI. Dependencies are fully pinned with
hashes in `requirements-docs.txt`. Change the direct pins, run `make docs-lock`,
and review the resulting lock before upgrading tools. Normal site builds do
not install tools or rerun benchmarks.

`make verify` also checks the original claim-ledger revision, fixture hashes,
coverage identities, capability tables, and complete roadmap inventory.
Public measurements live in [Evidence](EVIDENCE.md), not copied README tables.

## Publish

The Documentation workflow builds pull requests and deploys main through GitHub
Pages in this same repository. Configure Pages to use GitHub Actions and the
`github-pages` environment. No separate hosting repository or registry token
is involved. Successful main Benchmark artifacts can refresh the result page;
only validated data is imported, while executable site code comes from main.
The committed snapshot is the reproducible fallback for ordinary docs builds.

Keep every public guide reachable through `documentation.json` and site
navigation. Review the package file list before a release.
