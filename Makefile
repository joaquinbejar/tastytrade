# Makefile for common tasks in a Rust project
# Detect current branch
CURRENT_BRANCH := $(shell git rev-parse --abbrev-ref HEAD)

# Default target
.PHONY: all
all: test fmt lint build

# Build the project
# Every gate covers the whole workspace: the library, the CLI and the example
# crates all ship from this repository, so a gate that only sees the root
# package validates something the repository does not publish.
.PHONY: build
build:
	cargo build --workspace --all-targets --all-features

.PHONY: release
release:
	cargo build --workspace --release

# Run tests
.PHONY: test
test:
	LOGLEVEL=WARN cargo test --workspace --all-features

# Build wasm target
.PHONY: wasm-build
wasm-build:
	cargo +nightly build --target wasm32-unknown-unknown --release

# Test wasm target
.PHONY: wasm-test
wasm-test:
	cargo +nightly test --target wasm32-unknown-unknown --release

# Format the code
.PHONY: fmt
fmt:
	cargo +stable fmt --all

# Check formatting. Read-only: it reports a dirty diff, it never rewrites.
.PHONY: fmt-check
fmt-check:
	cargo +stable fmt --all --check

# Run Clippy for linting
.PHONY: lint
lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

.PHONY: lint-wasm
lint-wasm:
	cargo +nightly clippy --target wasm32-unknown-unknown --release --all-features -- -D warnings

.PHONY: lint-fix
lint-fix:
	cargo clippy --fix --workspace --all-targets --all-features --allow-dirty --allow-staged -- -D warnings

.PHONY: lint-wasm-fix
lint-wasm-fix:
	cargo +nightly clippy --fix --target wasm32-unknown-unknown --release --all-features --allow-dirty --allow-staged -- -D warnings

# Clean the project
.PHONY: clean
clean:
	cargo clean

# Pre-push checks. Every target here is read-only, so CI and a developer see
# the same verdict and neither of them rewrites the tree to reach it.
.PHONY: check
check: fmt-check lint test check-spanish doc-check

# Run the project
.PHONY: run
run:
	cargo run

.PHONY: fix
fix:
	cargo fix --allow-staged --allow-dirty

.PHONY: pre-push
pre-push: fix fmt lint-fix test readme

# Reading the docs. Opens a browser, so it is not a gate.
.PHONY: doc
doc: doc-check
	cargo doc --open

# Validating them. missing_docs is denied at the crate root, so `make lint`
# catches an item with no documentation at all; this catches documentation
# that is broken, which the presence check cannot see. Non-interactive, whole
# workspace, all features, warnings denied.
.PHONY: doc-check
doc-check:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

.PHONY: publish
publish: readme coverage
	cargo login ${CARGO_REGISTRY_TOKEN}
	cargo package
	cargo publish

.PHONY: coverage
coverage:
	export LOGLEVEL=WARN
	cargo install cargo-tarpaulin
	mkdir -p coverage
	cargo tarpaulin --all-features --workspace --timeout 120 --out Xml --output-dir coverage

.PHONY: coverage-html
coverage-html:
	export LOGLEVEL=WARN
	cargo install cargo-tarpaulin
	mkdir -p coverage
	cargo tarpaulin --all-features --workspace --timeout 120 --out Html --output-dir coverage

.PHONY: coverage-json
coverage-json:
	export LOGLEVEL=WARN
	cargo install cargo-tarpaulin
	mkdir -p coverage
	cargo tarpaulin --all-features --workspace --timeout 120 --out Json --output-dir coverage

.PHONY: open-coverage
open-coverage:
	open coverage/tarpaulin-report.html

# Rule to show git log
git-log:
	@if [ "$(CURRENT_BRANCH)" = "HEAD" ]; then \
		echo "You are in a detached HEAD state. Please check out a branch."; \
		exit 1; \
	fi; \
	echo "Showing git log for branch $(CURRENT_BRANCH) against main:"; \
	git log main..$(CURRENT_BRANCH) --pretty=full

.PHONY: create-doc
create-doc:
	cargo doc --no-deps --document-private-items

.PHONY: readme
readme: check-cargo-readme create-doc
	cargo readme > README.md

# Dependency advisories, licences, sources and version skew. Kept out of
# `check` on purpose: it needs the network and the RustSec database, and the
# pre-push gate must stay usable offline. CI runs it on every push and weekly.
.PHONY: check-cargo-deny
check-cargo-deny:
	@command -v cargo-deny > /dev/null || (echo "Installing cargo-deny..."; cargo install cargo-deny --locked)

.PHONY: deny
deny: check-cargo-deny deny-expiry
	cargo deny check

# cargo-deny has no expiring-ignore feature, so the `expires=YYYY-MM-DD` marker
# in each exception's reason is enforced here. An exception that outlives its
# date fails the build, which forces the decision to be taken again rather than
# inherited. ISO dates compare correctly as strings, so no date arithmetic.
.PHONY: deny-expiry
deny-expiry:
	@dates=$$(grep -oE 'expires=[0-9]{4}-[0-9]{2}-[0-9]{2}' deny.toml | cut -d= -f2); \
	ignores=$$(grep -cE '^[[:space:]]*\{ id = "RUSTSEC' deny.toml); \
	found=$$(printf '%s\n' "$$dates" | grep -c '[0-9]' || true); \
	if [ "$$found" != "$$ignores" ]; then \
		echo "deny-expiry: $$ignores exception(s) but $$found expiry marker(s)."; \
		echo "Every ignore needs owner=<login> expires=<YYYY-MM-DD> in its reason."; \
		exit 1; \
	fi; \
	today=$$(date -u +%Y-%m-%d); \
	expired=$$(printf '%s\n' "$$dates" | awk -v today="$$today" 'length($$0) > 0 && $$0 < today'); \
	if [ -n "$$expired" ]; then \
		echo "deny-expiry: advisory exception(s) expired on: $$expired"; \
		echo "Re-justify in deny.toml with a new date and owner, or drop the dependency."; \
		exit 1; \
	fi; \
	echo "deny-expiry: $$ignores exception(s), none expired"

# README.md is generated from src/lib.rs. Nothing stopped the two drifting,
# and a hand edit to the README silently disappears on the next regeneration.
.PHONY: readme-check
readme-check: check-cargo-readme create-doc
	@cargo readme > /tmp/tastytrade-readme-check.md
	@if ! diff -q README.md /tmp/tastytrade-readme-check.md > /dev/null; then \
		echo "README.md is out of date with src/lib.rs. Run: make readme"; \
		diff README.md /tmp/tastytrade-readme-check.md | head -40; \
		exit 1; \
	fi; \
	echo "readme-check: README.md matches src/lib.rs"

.PHONY: check-cargo-readme
check-cargo-readme:
	@command -v cargo-readme > /dev/null || (echo "Installing cargo-readme..."; cargo install cargo-readme --locked)

# Code and comments are English, across every crate this repository ships.
# This replaces the old scripts/spanish.py, which was referenced by the
# Makefile but never existed. Two conservative signals over comment lines
# only: Spanish-only punctuation and accents, and a word list with no English
# collisions. The author header is the one legitimate accented line.
SPANISH_ROOTS := src cli/src examples/*/src
SPANISH_WORDS := que|los|las|una|unos|unas|para|con|pero|desde|hasta|cuando|donde|tambien|ademas|aqui|puede|debe|este|esta|esto|estos|estas|del|por|como|mas|asi|solo|cada|sobre|entre|hacer|tiene|ser|codigo|respuesta|solicitud|ejemplo|advertencia|cuenta|error de

.PHONY: check-spanish
check-spanish:
	@comments=$$(grep -rn --include='*.rs' -E '^[[:space:]]*//' $(SPANISH_ROOTS) | grep -vE 'Joaqu|Author:|Email:'); \
	hits=$$( { printf '%s\n' "$$comments" | grep -iE '[áéíóúñ¿¡]'; \
	           printf '%s\n' "$$comments" | grep -iwE '($(SPANISH_WORDS))'; } | sort -u | grep -v '^$$' ); \
	if [ -n "$$hits" ]; then \
		echo "check-spanish: Spanish found in comments:"; \
		printf '%s\n' "$$hits"; \
		exit 1; \
	fi; \
	echo "check-spanish: clean"

.PHONY: zip
zip:
	@echo "Creating $(ZIP_NAME) without any 'target' directories, 'Cargo.lock', and hidden files..."
	@find . -type f \
		! -path "*/target/*" \
		! -path "./.*" \
		! -name "Cargo.lock" \
		! -name ".*" \
		| zip -@ $(ZIP_NAME)
	@echo "$(ZIP_NAME) created successfully."


.PHONY: check-cargo-criterion
check-cargo-criterion:
	@command -v cargo-criterion > /dev/null || (echo "Installing cargo-criterion..."; cargo install cargo-criterion)

.PHONY: bench
bench: check-cargo-criterion
	cargo criterion --output-format=quiet

.PHONY: bench-show
bench-show:
	open target/criterion/report/index.html

.PHONY: bench-save
bench-save: check-cargo-criterion
	cargo criterion --output-format quiet --history-id v0.3.2 --history-description "Version 0.3.2 baseline"

.PHONY: bench-compare
bench-compare: check-cargo-criterion
	cargo criterion --output-format verbose

.PHONY: bench-json
bench-json: check-cargo-criterion
	cargo criterion --message-format json

.PHONY: bench-clean
bench-clean:
	rm -rf target/criterion


.PHONY: workflow-coverage
workflow-coverage:
	DOCKER_HOST="$${DOCKER_HOST}" act push --job code_coverage_report \
       -P ubuntu-latest=catthehacker/ubuntu:latest \
       --privileged

.PHONY: workflow-build
workflow-build:
	DOCKER_HOST="$${DOCKER_HOST}" act push --job build \
       -P ubuntu-latest=catthehacker/ubuntu:latest

.PHONY: workflow-lint
workflow-lint:
	DOCKER_HOST="$${DOCKER_HOST}" act push --job lint

.PHONY: workflow-test
workflow-test:
	DOCKER_HOST="$${DOCKER_HOST}" act push --job run_tests

.PHONY: workflow
workflow: workflow-build workflow-lint workflow-test workflow-coverage
