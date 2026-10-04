.PHONY: serve desktop fmt clippy

serve:
	cd crates/dtap-app && dx serve

desktop:
	cd crates/dtap-app && dx serve --platform desktop

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets -- -D warnings
