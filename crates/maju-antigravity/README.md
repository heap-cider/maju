# maju-antigravity

Maju's built-in ACP adapter for the Google Antigravity `agy` CLI.

The adapter is bundled with Maju Desktop. It reuses the user's existing `agy`
installation and login, discovers models from `agy models`, exposes model and
model-specific reasoning choices through ACP `configOptions`, and translates
`stream-json` responses into ACP message, tool, and usage updates.

Long prompts are written to a temporary UTF-8 file and referenced from a short
command-line argument. This avoids Windows command-line length limits and shell
encoding differences without changing short-prompt behavior.

```bash
cargo test -p maju-antigravity
cargo build --release -p maju-antigravity
```

For tests or nonstandard installations, set `MAJU_ANTIGRAVITY_AGY_COMMAND` to
the `agy` executable path. `MAJU_ANTIGRAVITY_CACHE_DIR` overrides the model
cache location.
