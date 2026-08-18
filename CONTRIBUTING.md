# Contributing to vertify

Thanks for wanting to improve vertify. Small, focused changes are easier to review than large ones.

## Development setup

You need:

- [Rust](https://rustup.rs) (stable)
- [ffmpeg](https://ffmpeg.org) and `ffprobe` on your `PATH` (runtime; not required for unit tests)

```sh
git clone https://github.com/daylennguyen/vertify.git
cd vertify
cargo test --lib
cargo run -- --help
cargo run --bin vertify-gui
```

## What to work on

- Bugs and UX papercuts in the CLI or Flip Stage GUI
- Better defaults, clearer errors, and docs
- Tests for planning / filter construction (prefer these over brittle ffmpeg integration tests)
- Completions, installers, and packaging

Open an issue before starting a large feature so we can agree on the shape.

## Code style

- `cargo fmt --all`
- `cargo clippy --all-targets`
- Match existing naming and error-handling (`anyhow`, explicit `bail!`)
- Keep the conversion pipeline in `src/lib.rs` so the CLI and GUI share one implementation

## GUI visual tests

```sh
cargo test --test gui_visual -- --test-threads=1
```

Use a single test thread — wgpu snapshot rendering is not safe to parallelize. After an intentional UI change:

```sh
# Unix
UPDATE_SNAPSHOTS=1 cargo test --test gui_visual -- --test-threads=1

# PowerShell
$env:UPDATE_SNAPSHOTS="1"; cargo test --test gui_visual -- --test-threads=1
```

Commit updated baselines in `tests/snapshots/` with the UI change. Do not commit `*.diff.png` or `*.new.png`.

## Shell completions

After changing CLI flags:

```sh
cargo build --release --bin vertify
# Unix Makefile:
make completions

# Or by hand:
./target/release/vertify --completions bash > completions/vertify.bash
./target/release/vertify --completions zsh  > completions/_vertify
./target/release/vertify --completions fish > completions/vertify.fish
```

## Pull requests

- One concern per PR
- Include tests when behavior changes
- Update `CHANGELOG.md` under an `Unreleased` section for user-facing changes
- Fill in the PR template

By contributing you agree that your work is licensed under the MIT License in this repository.
