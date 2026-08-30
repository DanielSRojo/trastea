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

# --all-targets so the #[cfg(test)] modules are linted too. --locked, here and on
# test/build/build-release, makes a stale Cargo.lock a failure rather than something
# cargo quietly rewrites — `run` and `fmt` stay unlocked so the inner loop is unaffected.
lint:
    cargo clippy --locked --all-targets -- -D warnings

# unit tests, no UI harness needed
test:
    cargo test --locked

build:
    cargo build --locked

# the optimized binary the release workflow packages — its flags live here, not in the workflow
build-release:
    cargo build --locked --release

# launch the app
run:
    cargo run

# the third-party notices the release stages beside LICENSE — needs cargo-about installed
notices:
    cargo about generate about.hbs -o THIRD-PARTY-NOTICES.txt

# the release body the workflow publishes, for the tag at HEAD — needs git-cliff installed
notes:
    git-cliff --current --output RELEASE_NOTES.md

# the same notes for a version that is not tagged yet, to stdout — the pre-tag dry run
notes-preview version:
    git-cliff --unreleased --tag v{{version}}

# advisories and yanked crates across everything Cargo.lock names — needs cargo-deny
# installed. Deliberately not a stage of `ci`: the gate stays runnable with a Rust toolchain
# and nothing else, the same reason `notices` sits outside it.
deny:
    cargo deny check advisories

# the gate CI runs, in CI's order — dependencies run in sequence and stop at the first failure
ci: fmt-check lint test build
