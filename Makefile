# vertify — build / install / rebuild helpers
PREFIX ?= $(HOME)/.local
BINDIR  = $(PREFIX)/bin
BIN     = target/release/vertify
GUI     = target/release/vertify-gui

.PHONY: build install uninstall rebuild reinstall completions check clean

## Build optimized release binaries (CLI + GUI)
build:
	cargo build --release --locked --bins

## Build and copy binaries to $(BINDIR) (default: ~/.local/bin)
install: build
	install -d $(BINDIR)
	install -m 755 $(BIN) $(BINDIR)/vertify
	install -m 755 $(GUI) $(BINDIR)/vertify-gui
	@echo "Installed to $(BINDIR)/vertify and $(BINDIR)/vertify-gui"
	@command -v ffmpeg >/dev/null || echo "WARNING: ffmpeg not found on PATH — vertify needs it at runtime"

## Remove the installed binaries
uninstall:
	rm -f $(BINDIR)/vertify $(BINDIR)/vertify-gui

## Force a full rebuild from scratch
rebuild: clean build

## Rebuild and reinstall in one step (use after editing the source)
reinstall: rebuild install

## Generate shell completions into ./completions/
completions: build
	mkdir -p completions
	$(BIN) --completions bash > completions/vertify.bash
	$(BIN) --completions zsh  > completions/_vertify
	$(BIN) --completions fish > completions/vertify.fish

## Type-check and lint quickly without a full build
check:
	cargo check --locked
	cargo clippy --all-targets --locked

clean:
	cargo clean
