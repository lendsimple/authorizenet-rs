# Releasing

`authorizenet` and `authorizenet-derive` are released together, with the same version.
`authorizenet` pins `authorizenet-derive` exactly (`=x.y.z`), because the derives
generate calls into `authorizenet`'s internals.

Releases are published by the [Release workflow](.github/workflows/release.yml) when a
`v*` tag is pushed. It checks that the tag matches the crate version and that
`CHANGELOG.md` has a section for it, runs the tests, and then runs
`cargo publish --workspace`, which publishes `authorizenet-derive` first and skips the
unpublished `xtask`.

## Each release

1. Make sure `main` is green in CI.
2. Bump `version` in the workspace `Cargo.toml`, and the `authorizenet-derive`
   requirement in `authorizenet/Cargo.toml` to match.
3. In `CHANGELOG.md`, rename `[Unreleased]` to the new version with today's date, add
   an empty `[Unreleased]` section above it, and update the links at the bottom.
4. Check locally:

   ```sh
   cargo test --workspace --all-features
   cargo publish --dry-run --workspace
   ```

5. Commit, tag and push. The tag starts the release:

   ```sh
   git commit -am "Release x.y.z"
   git tag vx.y.z
   git push origin main vx.y.z
   ```

6. If the `release` environment requires approval, approve the run in the Actions tab.

If the workflow publishes `authorizenet-derive` but then fails on `authorizenet`, fix
the cause and publish the rest by hand (`cargo publish -p authorizenet`): re-running the
workflow would fail on the version that is already published.

## One-time setup

crates.io only allows trusted publishing (short-lived tokens, no stored secret) for
crates that already exist, so the first release needs an API token.

1. In the repository settings, create an environment named `release`. Adding required
   reviewers makes every release wait for approval.
2. Create a crates.io API token with the `publish-new` and `publish-update` scopes, and
   add it to the `release` environment as the secret `CARGO_REGISTRY_TOKEN`.
3. Release `0.1.0` as above. The workflow uses the secret when it exists.
4. On crates.io, for **each** of `authorizenet` and `authorizenet-derive`, open
   Settings → Trusted Publishing and add a GitHub publisher:
   - repository owner: `lendsimple`
   - repository name: `authorizenet-rs`
   - workflow filename: `release.yml`
   - environment: `release`
5. Delete the `CARGO_REGISTRY_TOKEN` secret, and revoke the token on crates.io. Later
   releases authenticate with trusted publishing.

To run the sandbox tests before a release:

```sh
ANET_LOGIN_ID=... ANET_TRANSACTION_KEY=... cargo test --all-features --test sandbox -- --ignored
```
