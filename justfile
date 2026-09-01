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

# Six glyphs out of a 990 KiB font, so what is committed under assets/ is the subset rather
# than the original and this recipe is how anyone reproduces it. The range is the one
# `every_keycap_is_one_the_embedded_subset_carries` checks the table against. The family name survives
# the subset, which is what `KEYCAP_FONT` matches on.
#
# rebuild the keycap subset from the system Noto Sans Math — needs fonttools installed
fonts:
    pyftsubset /usr/share/fonts/noto/NotoSansMath-Regular.ttf \
        --unicodes="U+2190-2193,U+21E4,U+21E5" \
        --output-file=assets/fonts/NotoSansMath-Keycaps.ttf

# the third-party notices the release stages beside LICENSE — needs cargo-about installed
notices:
    cargo about generate about.hbs -o THIRD-PARTY-NOTICES.txt

# the release body the workflow publishes, for the tag at HEAD — needs git-cliff installed
notes:
    git-cliff --current --output RELEASE_NOTES.md

# the same notes for a version that is not tagged yet, to stdout — the pre-tag dry run
notes-preview version:
    git-cliff --unreleased --tag v{{version}}

# publish to crates.io — the release workflow runs this with a short-lived trusted-publishing
# token in CARGO_REGISTRY_TOKEN. Verifies by building the packaged crate before it uploads,
# which is the last chance to catch a tarball that is missing a file the build needs.
publish:
    cargo publish --locked

# advisories and yanked crates across everything Cargo.lock names — needs cargo-deny
# installed. Deliberately not a stage of `ci`: the gate stays runnable with a Rust toolchain
# and nothing else, the same reason `notices` sits outside it.
deny:
    cargo deny check advisories

# the gate CI runs, in CI's order — dependencies run in sequence and stop at the first failure
ci: fmt-check lint test build
