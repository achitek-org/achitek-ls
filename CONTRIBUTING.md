# Contributing

## Getting Started

Clone the repository:

```sh
git clone https://github.com/achitek-org/achitek-ls.git
cd achitek-ls
```

This project uses Nix to provide a reproducible development environment. Enter
the shell with:

```sh
nix develop
```

If you use `direnv`, the repository includes an `.envrc` that loads the same
flake automatically:

```sh
direnv allow
```

The Nix shell includes the Rust toolchain, `cargo-nextest`, `cargo-watch`,
`just`, `rust-analyzer`, `lefthook`, and the native dependencies needed by the
crate.

## Task Runner

The project uses a `justfile` as a small task runner.

List available recipes:

```sh
just
```

Common recipes:

```sh
just check
just fmt
just fmt-check
just clippy
just test
just build
```

Before opening a pull request, run:

```sh
just pre-commit
```

`just ci` is currently an alias for the same checks.

## Running Tests

The preferred test command is:

```sh
just test
```

That runs the test suite through `cargo nextest`. If you are outside the Nix
shell and already have the required tools installed, the equivalent direct
command is:

```sh
cargo nextest run --all-features
```

For quick local iteration, regular Cargo tests are also useful:

```sh
cargo test
```

## Formatting And Lints

Format code with:

```sh
just fmt
```

Check formatting without modifying files:

```sh
just fmt-check
```

Run Clippy with warnings treated as errors:

```sh
just clippy
```

## Project Context

Before making architectural or feature-level changes, read:

- [Architecture](docs/ARCHITECTURE.md)
  Describes the active crate layout, module boundaries, request flow, document
  state, template awareness, and logging.
- [Capabilities](docs/CAPABILITIES.md)
  Tracks implemented LSP capabilities and useful future capabilities.

Use `docs/CAPABILITIES.md` when looking for potential features to implement. It
is also a good place to update when a change adds, removes, or meaningfully
changes an editor capability.

## Development Notes

- Keep protocol handling in `server`.
- Keep language meaning in `analysis`.
- Keep parsing and source ranges in `syntax`.
- Prefer focused handlers and small helper functions over large request
  dispatch blocks.
- Use `indoc` for multiline test fixtures.
- Logs must go to stderr, never stdout, because stdout is reserved for LSP
  protocol messages.

## AI Tools

AI-assisted contributions are welcome. If you use AI tools to write or revise
code, docs, tests, or commit messages, please review the output carefully before
submitting it.
