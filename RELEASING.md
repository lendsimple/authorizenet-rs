# Releasing

`authorizenet` and `authorizenet-derive` are released together, with the same version.
`authorizenet` pins `authorizenet-derive` exactly (`=x.y.z`), because the derives
generate calls into `authorizenet`'s internals.

1. Make sure `main` is green in CI.
2. Bump `version` in the workspace `Cargo.toml`, and the `authorizenet-derive`
   requirement in `authorizenet/Cargo.toml` to match.
3. In `CHANGELOG.md`, rename `[Unreleased]` to the new version with today's date, add
   an empty `[Unreleased]` section above it, and update the links at the bottom.
4. Check the packages:

   ```sh
   cargo xtask codegen --check
   cargo test --workspace --all-features
   cargo publish --dry-run --workspace
   ```

5. Commit, tag and push:

   ```sh
   git commit -am "Release x.y.z"
   git tag vx.y.z
   git push origin main vx.y.z
   ```

6. Publish both crates. Cargo publishes `authorizenet-derive` first, and skips
   `xtask`, which is not published:

   ```sh
   cargo publish --workspace
   ```

Optionally, run the sandbox tests first with sandbox credentials:

```sh
ANET_LOGIN_ID=... ANET_TRANSACTION_KEY=... cargo test --all-features --test sandbox -- --ignored
```
