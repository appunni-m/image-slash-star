PYTHON ?= python3
PACKAGE_NAME := image-slash-star
PACKAGE_VERSION = $(shell cargo metadata --locked --no-deps --format-version 1 | $(PYTHON) -c 'import json,sys; d=json.load(sys.stdin); print(next(p["version"] for p in d["packages"] if p["name"]=="image-slash-star"))')
RELEASE_DIR := target/release-artifacts
RELEASE_CRATE = $(RELEASE_DIR)/$(PACKAGE_NAME)-$(PACKAGE_VERSION).crate
REGISTRY_CRATE = $(RELEASE_DIR)/registry/$(PACKAGE_NAME)-$(PACKAGE_VERSION).crate
COVERAGE_TOOLCHAIN ?= nightly-2026-07-16
COVERAGE_REPORT ?= target/release-evidence/coverage.json
COVERAGE_CASES ?=
SELECTED_COVERAGE_REPORT ?=
COVERAGE_EXPORTER_PREFIX ?= $(CURDIR)/target/coverage-exporter/llvm22.1.8
COVERAGE_EXPORTER_SOURCE ?=
COVERAGE_EXPORTER_TARGET ?= coverage
# Quote optional launcher arguments for the recipe shell, including apostrophes.
coverage_exporter_quote = '$(subst ','"'"',$(1))'
# A literal line feed for rejecting recipe-breaking report names before the shell.
define coverage_exporter_newline


endef

.DEFAULT_GOAL := help
.NOTPARALLEL: ci release-verify

.PHONY: help
help:
	@printf "image-slash-star\n\n"
	@printf "  make build            Compile the locked default-feature crate\n"
	@printf "  make docs-setup       Install hash-locked documentation tools\n"
	@printf "  make docs-build       Build and validate the Pages site\n"
	@printf "  make docs-serve       Preview the site at localhost:8000\n"
	@printf "  make bench-setup      Build the pinned TurboJPEG benchmark oracle\n"
	@printf "  make bench            Run the complete JPEG comparison (BENCH_OUTPUT, BENCH_TARGET_DIR, BENCH_ROUNDS)\n"
	@printf "  make fmt              Check formatting\n"
	@printf "  make verify           Run generated, claim, roadmap, and legal checks\n"
	@printf "  make refresh-fixture-integrity Refresh current fixture receipts, then verify them\n"
	@printf "  make lint             Run strict Clippy for debug/release, coverage hooks, and benchmark code\n"
	@printf "  make lint-x86-simd    Lint the x86-64 SSE2 baseline and AVX2 builds\n"
	@printf "  make test             Run docs, tests, target feature lanes, and WASI Clippy\n"
	@printf "  make supply-chain     Run cargo-deny\n"
	@printf "  make coverage         Require alpha floors: lines 59, branches 46, functions 52, regions 58 percent\n"
	@printf "  make coverage-selected COVERAGE_CASES='decode:webp:<id>' Run exact parity/fault cases with a local LLVM report\n"
	@printf "  make coverage-exporter-setup Build the optional pinned native exporter (macOS ARM64; Git/CMake/Ninja, network on first setup)\n"
	@printf "  make coverage-with-exporter Use the verified exporter; COVERAGE_EXPORTER_TARGET=coverage-complete retains strict 100%%\n"
	@printf "  make package-verify   Build and consume a reproducible package\n"
	@printf "  make ci               Run all local CI gates, including workflow validation\n"
	@printf "  make release-verify   Require a clean, complete release candidate\n"
	@printf "  make dependency-update Update one dependency in both lockfiles (DEPENDENCY, DEPENDENCY_VERSION)\n"
	@printf "  make release-lock-update Refresh both workspace lockfiles after a version bump\n"
	@printf "  make release-registry-verify Compare the candidate with its published archive\n"
	@printf "  make release-publish-request Frame the verified crate archive for registry upload\n"
	@printf "  make coverage-complete Require 100 percent coverage across all four metrics\n"

.PHONY: fmt
fmt:
	cargo fmt --all -- --check
	cargo fmt --manifest-path benchmarks/jpeg-production/rust/Cargo.toml -- --check

.PHONY: verify-fixture-integrity
verify-fixture-integrity:
	$(PYTHON) scripts/verify_claim_ledger.py
	$(PYTHON) scripts/generate_malformed_ledger.py --check
	$(PYTHON) scripts/verify_coverage_origins.py
	$(PYTHON) scripts/verify_webp_vp8l_property_map.py
	$(PYTHON) scripts/verify_roadmap.py
	$(PYTHON) scripts/generate_capability_docs.py --check

.PHONY: refresh-fixture-integrity
refresh-fixture-integrity:
	$(PYTHON) scripts/refresh_fixture_integrity.py
	$(MAKE) verify-fixture-integrity

.PHONY: verify
verify: release-tools-test docs-lint docs-test verify-fixture-integrity
	$(PYTHON) scripts/verify_no_source_unit_tests.py
	$(PYTHON) scripts/verify_third_party_licenses.py
	$(PYTHON) scripts/generate_png_srgb_intent_fixtures.py --check
	$(PYTHON) scripts/generate_png_1bit_odd_width_fixture.py --check
	$(PYTHON) scripts/generate_png_adam7_rgb16_fixture.py --check
	$(PYTHON) scripts/generate_png_signature_nearmiss_fixture.py --check
	$(PYTHON) scripts/generate_png_itxt_missing_keyword_separator.py --check
	$(PYTHON) scripts/generate_png_idat_split_zlib_header_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_trailing_dri_fixture.py --check
	$(PYTHON) scripts/generate_png_interrupted_idat_apng_fixture.py --check
	$(PYTHON) scripts/generate_png_apng_duplicate_actl.py --check
	$(PYTHON) scripts/generate_png_apng_fdat_sequence_mismatch_before_idat.py --check
	$(PYTHON) scripts/generate_png_apng_gray16_partial_fixture.py --check
	$(PYTHON) scripts/generate_ico_equal_area_tiebreak_fixture.py --check
	$(PYTHON) scripts/generate_ico_equal_area_same_depth_fixture.py --check
	$(PYTHON) scripts/generate_cur_equal_size_y_hotspots_fixture.py --check
	$(PYTHON) scripts/generate_cur_larger_dimensions_fixture.py --check
	$(PYTHON) scripts/generate_ico_zero_bpp_depth_fallbacks_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_long_huffman_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_trailing_dht_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_trailing_app1_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_com_tail_junk_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_com_eoi_marker_fill_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_com_ff00_tail_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_two_trailing_com_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_trailing_app2_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_trailing_app0_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_post_scan_app14_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_trailing_dht_malformed_fixtures.py --check
	$(PYTHON) scripts/generate_bmp_gif_edge_fixtures.py --check
	$(PYTHON) scripts/generate_bmp_top_down_24_odd_width_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_unused_custom_dc_chroma_table_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_custom_active_ac_chroma_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_progressive_correction_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_ycck_subsampled_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_ycck_h2v2_singleton_fixture.py --check
	$(PYTHON) scripts/generate_tiff_duplicate_rational_fill_order_fixture.py --check
	$(PYTHON) scripts/generate_tiff_rgb16_fixtures.py --check
	$(PYTHON) scripts/generate_tiff_palette4_fillorder2_fixtures.py --check
	$(PYTHON) scripts/generate_tiff_ycbcr_subsampled_fixtures.py --check
	$(PYTHON) scripts/generate_webp_animated_top_level_image_fixtures.py --check
	$(PYTHON) scripts/generate_webp_animated_origin_partial_fixture.py --check
	$(PYTHON) scripts/generate_webp_lossless_run_cap_fixture.py --check
	$(PYTHON) scripts/generate_webp_method0_sampling_boundary_fixture.py --check
	$(PYTHON) scripts/generate_avif_iinf_child_kind_fixture.py --check
	$(PYTHON) scripts/generate_avif_iloc_trailing_payload_fixture.py --check
	$(PYTHON) scripts/generate_avif_multi_entry_stsc_fixture.py --check
	$(PYTHON) scripts/generate_avif_imir_fixtures.py --check
	$(PYTHON) scripts/generate_avif_iref_v1_alpha_fixture.py --check
	$(PYTHON) scripts/generate_avif_monochrome_grid_edges.py --check
	$(PYTHON) scripts/generate_avif_filmgrain_chroma_from_luma_odd.py --check
	$(PYTHON) scripts/generate_avif_inter_i444_frame_header_b64.py --check
	$(PYTHON) scripts/verify_diagnostic_provenance.py
	$(PYTHON) scripts/verify_unreachable_contracts.py

.PHONY: lint
# All-features can hide cfg branches used by the crate's default feature set.
lint:
	cargo clippy --workspace --all-targets --locked -- -D warnings
	cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
	cargo clippy --release --workspace --all-targets --all-features --locked -- -D warnings
	CARGO_TARGET_DIR=target/cfg-coverage-clippy cargo +"$(COVERAGE_TOOLCHAIN)" clippy --workspace --all-targets --all-features --locked -- -D warnings --cfg coverage --cfg coverage_nightly
	cargo clippy --manifest-path benchmarks/jpeg-production/rust/Cargo.toml --release --locked -- -D warnings

.PHONY: lint-x86-simd
lint-x86-simd:
	$(MAKE) fmt
	CARGO_TARGET_DIR=target/x86_64-sse2 RUSTFLAGS="-C target-cpu=x86-64" cargo clippy --workspace --all-targets --all-features --locked --target x86_64-unknown-linux-gnu -- -D warnings
	CARGO_TARGET_DIR=target/x86_64-avx2 RUSTFLAGS="-C target-cpu=x86-64 -C target-feature=+avx2" cargo clippy --workspace --all-targets --all-features --locked --target x86_64-unknown-linux-gnu -- -D warnings

.PHONY: test
test:
	cargo check --all-features --locked
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
	cargo test --doc --all-features --locked
	cargo test --workspace --all-features --locked
	$(MAKE) test-feature-matrix

.PHONY: test-feature-matrix
test-feature-matrix: ## Run native, WASI, and WASM lanes with strict target Clippy
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

.PHONY: coverage-selected
coverage-selected:
	@test -n "$(value COVERAGE_CASES)" || { printf "Set COVERAGE_CASES to one or more exact operation:format:row-id selectors.\\n" >&2; exit 2; }
	$(if $(findstring $$,$(value COVERAGE_CASES)),$(error COVERAGE_CASES contains a dollar sign unsupported by the Make recipe))
	$(if $(findstring `,$(value COVERAGE_CASES)),$(error COVERAGE_CASES contains a backtick unsupported by the Make recipe))
	$(if $(findstring ",$(value COVERAGE_CASES)),$(error COVERAGE_CASES contains a double quote unsupported by the Make recipe))
	$(if $(findstring \, $(value COVERAGE_CASES)),$(error COVERAGE_CASES contains a backslash unsupported by the Make recipe))
	$(if $(findstring $(coverage_exporter_newline),$(value COVERAGE_CASES)),$(error COVERAGE_CASES contains a newline unsupported by the Make recipe))
	$(if $(findstring $$,$(value SELECTED_COVERAGE_REPORT)),$(error SELECTED_COVERAGE_REPORT contains a dollar sign unsupported by the Make recipe))
	$(if $(findstring `,$(value SELECTED_COVERAGE_REPORT)),$(error SELECTED_COVERAGE_REPORT contains a backtick unsupported by the Make recipe))
	$(if $(findstring ",$(value SELECTED_COVERAGE_REPORT)),$(error SELECTED_COVERAGE_REPORT contains a double quote unsupported by the Make recipe))
	$(if $(findstring \, $(value SELECTED_COVERAGE_REPORT)),$(error SELECTED_COVERAGE_REPORT contains a backslash unsupported by the Make recipe))
	$(if $(findstring $(coverage_exporter_newline),$(value SELECTED_COVERAGE_REPORT)),$(error SELECTED_COVERAGE_REPORT contains a newline unsupported by the Make recipe))
	$(PYTHON) scripts/run_selected_coverage.py --cases $(call coverage_exporter_quote,$(value COVERAGE_CASES)) \
		--toolchain $(call coverage_exporter_quote,$(value COVERAGE_TOOLCHAIN)) \
		$(if $(value SELECTED_COVERAGE_REPORT),--report $(call coverage_exporter_quote,$(value SELECTED_COVERAGE_REPORT)))

.PHONY: coverage-exporter-setup coverage-with-exporter
coverage-exporter-setup:
	$(PYTHON) scripts/setup_coverage_exporter.py setup \
		--prefix $(call coverage_exporter_quote,$(COVERAGE_EXPORTER_PREFIX)) --source $(call coverage_exporter_quote,$(COVERAGE_EXPORTER_SOURCE)) \
		--toolchain $(call coverage_exporter_quote,$(COVERAGE_TOOLCHAIN))

coverage-with-exporter:
	$(if $(findstring $$,$(value COVERAGE_REPORT)),$(error COVERAGE_REPORT contains characters unsupported by the recursive Make recipes))
	$(if $(findstring $(coverage_exporter_newline),$(value COVERAGE_REPORT)),$(error COVERAGE_REPORT contains characters unsupported by the recursive Make recipes))
	+$(PYTHON) scripts/setup_coverage_exporter.py run \
		--prefix $(call coverage_exporter_quote,$(COVERAGE_EXPORTER_PREFIX)) --toolchain $(call coverage_exporter_quote,$(COVERAGE_TOOLCHAIN)) \
		--make $(call coverage_exporter_quote,$(MAKE)) --target $(call coverage_exporter_quote,$(COVERAGE_EXPORTER_TARGET)) --report $(call coverage_exporter_quote,$(value COVERAGE_REPORT))

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
ci: workflows-check fmt verify lint test supply-chain coverage package-verify

.PHONY: release-verify
release-verify:
	@test -z "$$(git status --porcelain=v1 --untracked-files=all)" || { \
		printf "release-verify requires a clean worktree.\n" >&2; exit 2; \
	}
	$(MAKE) ci

.PHONY: ci-quality
ci-quality: workflows-check fmt verify lint test package-verify

.PHONY: release-check-version
release-check-version:
	$(PYTHON) scripts/verify_release_archive.py --metadata-only

.PHONY: dependency-update
dependency-update:
	@test -n "$(DEPENDENCY)" -a -n "$(DEPENDENCY_VERSION)" || { \
		printf "Set DEPENDENCY and DEPENDENCY_VERSION to the reviewed package and exact version.\n" >&2; exit 2; \
	}
	cargo update --package "$(DEPENDENCY)" --precise "$(DEPENDENCY_VERSION)"
	cargo update --manifest-path benchmarks/jpeg-production/rust/Cargo.toml --package "$(DEPENDENCY)" --precise "$(DEPENDENCY_VERSION)"

.PHONY: release-lock-update
release-lock-update:
	cargo update --workspace --offline
	cargo update --manifest-path benchmarks/jpeg-production/rust/Cargo.toml --workspace --offline

.PHONY: release-registry-verify
release-registry-verify:
	$(PYTHON) scripts/publish_release.py --candidate "$(RELEASE_CRATE)" --output "$(REGISTRY_CRATE)"

.PHONY: release-publish-prepare
release-publish-prepare:
	cargo package --locked
	cmp "target/package/$(PACKAGE_NAME)-$(PACKAGE_VERSION).crate" "$(VERIFIED_CRATE)"

.PHONY: release-publish-request
release-publish-request:
	@test -n "$(VERIFIED_CRATE)" && test -n "$(PUBLISH_REQUEST)" || { \
		printf "Set VERIFIED_CRATE and PUBLISH_REQUEST to the verified crate and upload request paths.\n" >&2; exit 2; \
	}
	$(PYTHON) scripts/prepare_crates_io_publish.py --candidate "$(VERIFIED_CRATE)" --output "$(PUBLISH_REQUEST)"

.PHONY: build fmt-fix example bench bench-setup
build:
	cargo build --locked

fmt-fix:
	cargo fmt --all
	cargo fmt --manifest-path benchmarks/jpeg-production/rust/Cargo.toml

example:
	cargo run --locked --example package_smoke

BENCH_OUTPUT ?= target/benchmarks/jpeg/latest
BENCH_ROUNDS ?= 5
BENCH_TARGET_DIR ?= target/jpeg-production-matrix
TURBOJPEG_PREFIX ?= $(CURDIR)/target/benchmark-oracle/3.2.0/install
bench-setup:
	$(PYTHON) scripts/setup_benchmark_oracle.py

bench:
	JPEG_PRODUCTION_TARGET_DIR="$(BENCH_TARGET_DIR)" \
	  $(PYTHON) benchmarks/jpeg-production/run_matrix.py --rounds "$(BENCH_ROUNDS)" \
	  --output "$(BENCH_OUTPUT)" --turbojpeg-prefix "$(TURBOJPEG_PREFIX)"

include docs.mk

.PHONY: package-surface-check
package-surface-check: ## Check the exact declared Cargo archive file list
	$(PYTHON) scripts/verify_package_surface.py

.PHONY: workflows-check
workflows-check: ## Require SHA-pinned actions and validate workflows with actionlint
	$(PYTHON) scripts/check_workflows.py
