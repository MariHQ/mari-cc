# Install Mari

The Mari binary is a single Rust executable that runs as a Claude Code plugin. It downloads no models at install time and has no runtime service. It fetches the two local models it uses on first use, not during setup.

## Build from source

We are still setting up prebuilt binaries and an install channel. Until then, build from source. You need a Rust toolchain and `cmake`, since llama.cpp builds from source.

```sh
cargo install --path .
# or
cargo build --release   # binary at target/release/mari
```

Confirm the binary is on your `PATH`:

```sh
mari --version
```

## Wire it into Claude Code

Mari ships as a Claude Code plugin. The `skills/`, `commands/`, and `hooks/` directories, plus `.claude-plugin/plugin.json`, wrap the `mari` binary. For the plugin to work, `mari` must be on your `PATH`. Reload Claude Code after installing so it picks up the commands and the post-edit hook.

Once wired, the standalone slash commands work directly: `/search`, `/sync`, `/tag`, `/factcheck`, `/audit`, `/deslop`, `/tighten`, `/clarify`, `/sharpen`, `/understate`, `/critique`, `/polish`, and `/draft`. See the [slash command reference](../reference/slash-commands.md) for what each one maps to.

## Models on first use

Mari uses two small local models, downloaded the first time a command needs them into `~/.mari/models`:

- **Embeddings**: `Qwen3-Embedding-0.6B` (Apache-2.0), about 640 MB. Needed for `sync` and `search`.
- **Attention**: `Qwen3.5-0.8B` (Apache-2.0), about 520 MB. Needed only for the opt-in `--deep` grounding and coverage passes.

Check what's present with `mari model status`, or fetch both ahead of time with `mari model pull all`. An optional optical character recognition (OCR) tier for scanned PDFs is off by default and needs an explicit opt-in, because it runs code from the model repository. See `SECURITY.md` for the details. The [Models reference](../reference/models.md) covers the full picture.

## Verify the install

Run the environment check to see which optional tools and models are available:

```sh
mari doctor
```

Then head to the [Quickstart](quickstart.md) to index your first source.
