PYTHON ?= python3
PACKAGE_NAME := image-slash-star
PACKAGE_VERSION := $(shell cargo metadata --locked --no-deps --format-version 1 | $(PYTHON) -c 'import json,sys; d=json.load(sys.stdin); print(next(p["version"] for p in d["packages"] if p["name"]=="image-slash-star"))')
RELEASE_DIR := target/release-artifacts
RELEASE_CRATE := $(RELEASE_DIR)/$(PACKAGE_NAME)-$(PACKAGE_VERSION).crate
REGISTRY_CRATE := $(RELEASE_DIR)/registry/$(PACKAGE_NAME)-$(PACKAGE_VERSION).crate
COVERAGE_TOOLCHAIN ?= nightly-2026-07-16

.DEFAULT_GOAL := help
.NOTPARALLEL: ci release-verify

.PHONY: help
help:
	@printf "image-slash-star\n\n"
	@printf "  make fmt              Check formatting\n"
	@printf "  make verify           Run generated, claim, roadmap, and legal checks\n"
	@printf "  make lint             Run strict Clippy and benchmark lint\n"
	@printf "  make test             Run docs, tests, and target feature lanes\n"
	@printf "  make supply-chain     Run cargo-deny\n"
	@printf "  make coverage         Run the pinned four-metric coverage gate\n"
	@printf "  make package-verify   Build and consume a reproducible package\n"
	@printf "  make ci               Run all local CI gates\n"
	@printf "  make release-verify   Require a clean, complete release candidate\n"
	@printf "  make release-bootstrap Publish/verify the first crates.io version\n"

.PHONY: fmt
fmt:
	cargo fmt --all -- --check

.PHONY: verify
verify:
	$(PYTHON) scripts/verify_third_party_licenses.py
	$(PYTHON) scripts/generate_malformed_ledger.py --check
	$(PYTHON) scripts/verify_claim_ledger.py
	$(PYTHON) scripts/verify_coverage_origins.py
	$(PYTHON) scripts/verify_diagnostic_provenance.py
	$(PYTHON) scripts/verify_unreachable_contracts.py
	$(PYTHON) scripts/verify_webp_vp8l_property_map.py
	$(PYTHON) scripts/verify_roadmap.py
	$(PYTHON) scripts/generate_capability_docs.py --check

.PHONY: lint
lint:
	cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
	cargo fmt --manifest-path benchmarks/jpeg-production/rust/Cargo.toml -- --check
	cargo clippy --manifest-path benchmarks/jpeg-production/rust/Cargo.toml --release --locked -- -D warnings

.PHONY: test
test:
	cargo check --all-features --locked
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
	cargo test --doc --all-features --locked
	cargo test --workspace --all-features --locked
	scripts/test_feature_matrix.sh

.PHONY: supply-chain
supply-chain:
	cargo deny check

.PHONY: coverage
coverage:
	mkdir -p target/release-evidence
	cargo +"$(COVERAGE_TOOLCHAIN)" llvm-cov --all-features --branch --json \
		--output-path target/release-evidence/coverage.json --no-fail-fast
	$(PYTHON) scripts/verify_llvm_coverage.py target/release-evidence/coverage.json

.PHONY: package-verify
package-verify:
	$(PYTHON) scripts/verify_release_archive.py

.PHONY: ci
ci: fmt verify lint test supply-chain coverage package-verify

.PHONY: release-verify
release-verify:
	@test -z "$$(git status --porcelain=v1 --untracked-files=all)" || { \
		printf "release-verify requires a clean worktree.\n" >&2; exit 2; \
	}
	$(MAKE) ci

.PHONY: release-bootstrap
release-bootstrap: release-verify
	@test "$$RELEASE_APPROVED" = "1" || { \
		printf "set RELEASE_APPROVED=1 after reviewing release-verify\n" >&2; exit 2; \
	}
	@test "$$RELEASE_CI_SHA" = "$$(git rev-parse HEAD)" || { \
		printf "RELEASE_CI_SHA must equal the exact reviewed commit\n" >&2; exit 2; \
	}
	$(PYTHON) scripts/publish_release.py \
		--candidate "$(RELEASE_CRATE)" \
		--output "$(REGISTRY_CRATE)" \
		--publish-if-missing
