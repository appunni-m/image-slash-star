PYTHON ?= python3
PACKAGE_NAME := image-slash-star
PACKAGE_VERSION := $(shell cargo metadata --locked --no-deps --format-version 1 | $(PYTHON) -c 'import json,sys; d=json.load(sys.stdin); print(next(p["version"] for p in d["packages"] if p["name"]=="image-slash-star"))')
RELEASE_DIR := target/release-artifacts
RELEASE_CRATE := $(RELEASE_DIR)/$(PACKAGE_NAME)-$(PACKAGE_VERSION).crate
REGISTRY_CRATE := $(RELEASE_DIR)/registry/$(PACKAGE_NAME)-$(PACKAGE_VERSION).crate
COVERAGE_TOOLCHAIN ?= nightly-2026-07-16
COVERAGE_REPORT ?= target/release-evidence/coverage.json

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
	@printf "  make coverage         Require alpha floors: lines 59, branches 46, functions 52, regions 58 percent\n"
	@printf "  make package-verify   Build and consume a reproducible package\n"
	@printf "  make ci               Run all local CI gates\n"
	@printf "  make release-verify   Require a clean, complete release candidate\n"
	@printf "  make coverage-complete Require 100 percent coverage across all four metrics\n"

.PHONY: fmt
fmt:
	cargo fmt --all -- --check

.PHONY: verify
verify: release-tools-test
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
	cargo +"$(COVERAGE_TOOLCHAIN)" llvm-cov --all-features --branch --locked --json \
		--output-path "$(COVERAGE_REPORT)" --no-fail-fast
	$(MAKE) coverage-check

.PHONY: coverage-check
coverage-check:
	$(PYTHON) scripts/verify_llvm_coverage.py "$(COVERAGE_REPORT)"

.PHONY: coverage-complete
coverage-complete: coverage
	$(PYTHON) scripts/verify_llvm_coverage.py "$(COVERAGE_REPORT)" --strict

.PHONY: release-tools-test
release-tools-test:
	$(PYTHON) scripts/test_release_checks.py

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
release-bootstrap:
	@printf "The bootstrap is complete. Publish an annotated tag through GitHub OIDC.\n" >&2
	@exit 2

.PHONY: ci-quality
ci-quality: fmt verify lint test package-verify

.PHONY: release-check-version
release-check-version:
	$(PYTHON) scripts/verify_release_archive.py --metadata-only

.PHONY: release-publish-prepare
release-publish-prepare:
	cargo package --locked
	cmp "target/package/$(PACKAGE_NAME)-$(PACKAGE_VERSION).crate" "$(VERIFIED_CRATE)"

.PHONY: release-publish-oidc
release-publish-oidc:
	$(PYTHON) scripts/publish_release.py --candidate "$(VERIFIED_CRATE)" --output "$(REGISTRY_OUTPUT)" --publish-if-missing
