code-macos:
    code .vscode/rust-skia-macos.code-workspace

code-macos-gl:
    code .vscode/rust-skia-macos-gl.code-workspace

code-windows:
    code .vscode/rust-skia-windows.code-workspace

test-docs-rs-linux-arm:
        #!/usr/bin/env bash
        set -euo pipefail
        make bindings-docs
        cp /tmp/bindings.rs skia-bindings/bindings_docs.rs
        docker run --rm --platform linux/arm64 \
            -e DOCS_RS=1 \
            -v "$PWD:/work" \
            -w /work/skia-bindings \
            rust:latest \
            bash -lc '/usr/local/cargo/bin/cargo rustdoc -vv'
