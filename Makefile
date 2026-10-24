.PHONY: serve desktop fmt clippy gateway

serve:
	cd crates/dtap-app && dx serve --platform web

desktop:
	cd crates/dtap-app && dx serve --platform desktop

gateway:
	DTAP_MOCK=1 cargo run -p dtap-gateway

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets -- -D warnings
