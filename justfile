set shell := ["bash", "-cu"]
set windows-shell := ["powershell", "-NoProfile", "-Command"]

build:
    cargo build --release --manifest-path robotd-rs/Cargo.toml

run *args:
    cargo run --manifest-path robotd-rs/Cargo.toml -- {{args}}

test:
    cargo test --manifest-path robotd-rs/Cargo.toml
    cargo build --manifest-path robotd-rs/Cargo.toml
    uv run --extra remote python -m unittest discover -s robotd/tests -v
    uv run --extra remote python -m unittest discover -s robot_remote/tests -v

proto:
    uv run --extra dev python robotd/generate.py

check:
    cargo fmt --manifest-path robotd-rs/Cargo.toml --check
    cargo clippy --manifest-path robotd-rs/Cargo.toml --all-targets -- -D warnings
