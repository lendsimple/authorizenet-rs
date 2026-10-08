# Schema

`AnetApiSchema.xsd` is Authorize.Net's API schema, vendored so that code generation is
reproducible. `overrides.toml` holds every adjustment `cargo xtask codegen` makes on
top of it, each with the reason.

| | |
|---|---|
| Source | <https://api.authorize.net/xml/v1/schema/AnetApiSchema.xsd> |
| Downloaded | 2026-10-07 |
| SHA-256 | `c9c76cdbeba1f030bb453cdeec963457b9f85c6799db9b3888b0ade1a635a1fa` |

## Updating

1. Download the XSD over `AnetApiSchema.xsd` and update the date and hash above.
2. Run `cargo xtask codegen`. It fails, naming the problem, if the new schema needs
   an override (for example a name collision) or makes an override stale.
3. Run `cargo xtask drift --python <python-authorizenet checkout>` to refresh
   `docs/schema-drift.md`, and `cargo test` (fixtures are parsed strictly, so new
   required elements show up there).
