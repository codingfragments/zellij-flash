plugin_dir := env_var('HOME') + "/.config/zellij/plugins"
wasm_src   := "target/wasm32-wasip1/release/zellij_flash.wasm"
wasm_dst   := plugin_dir + "/zellij_flash.wasm"

default:
    @just --list

# Build release WASM
build:
    cargo build --release --target wasm32-wasip1

# Build dev WASM (faster, larger — good for iteration)
build-dev:
    cargo build --target wasm32-wasip1

# Copy WASM into ~/.config/zellij/plugins/
install: build
    mkdir -p {{plugin_dir}}
    cp -f {{wasm_src}} {{wasm_dst}}
    @echo "Installed:"
    @ls -lh {{wasm_dst}}

# Build + install + reload in a running Zellij session
dev: build
    cp -f {{wasm_src}} {{wasm_dst}}
    zellij action reload-plugin "file:{{wasm_dst}}" 2>/dev/null \
        || echo "Plugin not loaded yet — open it once with Ctrl-s S first"

# Build dev profile + install + reload (faster iteration cycle)
dev-fast: build-dev
    cp -f "target/wasm32-wasip1/debug/zellij_flash.wasm" {{wasm_dst}}
    zellij action reload-plugin "file:{{wasm_dst}}" 2>/dev/null \
        || echo "Plugin not loaded yet — open it once with Ctrl-s S first"

# Run cargo tests (host-native, no WASM target needed)
test:
    cargo test

# Run fmt + clippy + test + wasm build (mirrors CI exactly)
check:
    cargo fmt -- --check
    cargo clippy --all-targets -- -D warnings
    cargo test --locked
    cargo build --locked --release --target wasm32-wasip1
    @wc -c < target/wasm32-wasip1/release/zellij_flash.wasm | \
     awk 'BEGIN{lim=1536000} {printf "Binary: %d bytes (limit %d)\n",$1,lim; if($1>lim){print "ERROR: wasm exceeds 1.5 MB budget";exit 1}}'

# Format source
fmt:
    cargo fmt

clean:
    cargo clean

# Remove Zellij's compiled-module cache for zellij-flash.
# Zellij caches by load path, not content hash — run this after
# a zellij-tile version bump or any ABI-affecting change to force
# recompile on next launch.
clear-cache:
    find "$HOME/Library/Caches/org.Zellij-Contributors.Zellij/" \
        -path '*zellij_flash*' \
        -exec rm -rf {} + 2>/dev/null || true
    @echo "Cleared Zellij compiled-module cache for zellij-flash."
