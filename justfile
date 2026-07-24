# Justfile for org-cli

bin := "org-cli"
install_dir := env_var('HOME') / "bin"

# Default recipe: list available commands
default:
    @just --list

# Build the release binary
build:
    cargo build --release

# Run the test suite
test:
    cargo test

# Lint with clippy (warnings as errors)
lint:
    cargo clippy -- -D warnings

# Build release and install to ~/bin (on PATH, shadows the nix-profile copy for dev iteration)
install: build
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p "{{install_dir}}"
    install -m 0755 "target/release/{{bin}}" "{{install_dir}}/{{bin}}"
    echo "Installed {{bin}} -> {{install_dir}}/{{bin}}"
    echo "Using: $(command -v {{bin}})"

# Remove the ~/bin dev copy (falls back to the home-manager / nix-profile version)
uninstall:
    #!/usr/bin/env bash
    set -euo pipefail
    rm -f "{{install_dir}}/{{bin}}"
    echo "Removed {{install_dir}}/{{bin}}"
    if command -v {{bin}} >/dev/null 2>&1; then
        echo "Now using: $(command -v {{bin}})"
    else
        echo "{{bin}} no longer on PATH"
    fi

# Install via cargo to ~/.cargo/bin (portable; ensure ~/.cargo/bin is on PATH)
install-cargo:
    cargo install --path .

# Remove build artifacts
clean:
    cargo clean
