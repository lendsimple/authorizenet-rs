#!/usr/bin/env bash
# Runs the authorizenet test suite from the packaged crates, outside the repository,
# as crater and Linux distributions do: everything the tests need must be in the
# package. Extra arguments go to `cargo test`.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
version=$(cargo metadata --manifest-path "$root/Cargo.toml" --no-deps --format-version 1 \
  | jq -r '.packages[] | select(.name == "authorizenet") | .version')

cargo package --manifest-path "$root/Cargo.toml" --workspace --no-verify --allow-dirty

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
for crate in authorizenet authorizenet-derive; do
  tar -xzf "$root/target/package/$crate-$version.crate" -C "$work"
done

cd "$work/authorizenet-$version"
cat >> Cargo.toml <<TOML

# Test this crate on its own, with authorizenet-derive from its package, as if both
# had been downloaded from crates.io.
[workspace]

[patch.crates-io]
authorizenet-derive = { path = "../authorizenet-derive-$version" }
TOML

cargo test --all-features "$@"
