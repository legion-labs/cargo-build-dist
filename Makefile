check: check-format check-build check-clippy check-dockerize

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

check-dockerize:
	echo "cargo dockerize check"

check-clippy:
	cargo clippy --locked --all-targets --all-features -- -D warnings

dockerize:
	echo "cargo dockerize build"

build:
	cargo build

build-release:
	cargo build --release

clean:
	cargo clean