# the project's commands, in one place: CI runs `just ci`, so a local run and the
# workflow execute the same recipe text.

# a bare `just` prints this list rather than launching the app
[private]
default:
    @just --list

# format the source in place
fmt:
    cargo fmt --all

# fail if anything is unformatted — the form CI needs
fmt-check:
    cargo fmt --all --check

# --all-targets so the #[cfg(test)] modules are linted too
lint:
    cargo clippy --all-targets -- -D warnings

# unit tests, no UI harness needed
test:
    cargo test

build:
    cargo build

# launch the app
run:
    cargo run

# the gate CI runs, in CI's order — dependencies run in sequence and stop at the first failure
ci: fmt-check lint test build
