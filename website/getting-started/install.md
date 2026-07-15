# Install

Build and install the Rust CLI from the repository:

```sh
cargo install --path . --locked
```

Then add the checkout as a Claude Code marketplace and install the plugin:

```text
/plugin marketplace add /path/to/mari-cc
/plugin install mari@mari
```

Run `mari --version` to confirm the binary is available. Project settings live in `.mari/config.json` at the repository root.
