# authorizenet-derive

Derive macros used by the [`authorizenet`](https://crates.io/crates/authorizenet) crate
to map its Authorize.Net schema types to XML.

This crate is an implementation detail of `authorizenet`. Its generated code calls
`authorizenet`'s internals, and the two crates are released together with matching
versions, so depend on `authorizenet` instead.

Licensed under the MIT license.
