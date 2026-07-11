# Build helpers for mari + its embedded web console.
#
# The console (console/) is a Vite/React app served locally by `mari console`.
# Its built output (console/dist) is embedded into the binary via include_dir!
# at compile time and is committed to the repo, so a plain `cargo build` (or
# `cargo install --git`) works with no Node toolchain. Rebuild the console
# whenever the frontend changes:
#
#   make console      # install deps + build console/dist
#   make build        # console + release binary
#   make run          # launch the console (debug binary)

.PHONY: console console-deps build build-debug run clean-console

console-deps:
	cd console && npm install --no-audit --no-fund

console: console-deps
	cd console && npm run build

# Full release build: refresh the embedded console, then compile the binary.
build: console
	cargo build --release --bin mari

build-debug: console
	cargo build --bin mari

run:
	cargo run --bin mari -- console --open

clean-console:
	rm -rf console/node_modules console/dist
