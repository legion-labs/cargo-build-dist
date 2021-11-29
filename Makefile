all: check build test

check: check-format check-build check-clippy

check-env:
	rustup --version
	cargo deny --version
	mdbook --version
	cmake --version
	python3 --version
	ninja --version

check-format:
	cargo fmt --all -- --check

check-build:
	cargo check --locked --all-targets --all-features

check-clippy:
	cargo clippy --locked --all-targets --all-features -- -D warnings

build-all: build build-release

build:
	cargo build

build-release:
	cargo build --release

clean:
	cargo clean

test: test-build test-run

test-build:
	cargo test --no-run

test-run:
	cargo test
