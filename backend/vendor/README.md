# Patched dependencies

## imap-proto 0.10.2

Source: the crates.io `imap-proto` 0.10.2 archive (SHA-256
`16a6def1d5ac8975d70b3fd101d57953fe3278ef2ee5d7816cba54b1d1dfc22f`).
Upstream commit: `08f269400ec95b176d30cac7b23730820fe94d7e` in
<https://github.com/djc/tokio-imap> (now <https://github.com/djc/imap-proto>).
The MIT and Apache licenses come from that upstream commit.
`Cargo.toml` is the archive's original `Cargo.toml.orig`.

`imap` 2.4 requires this parser's 0.10 API. The `[patch.crates-io]` entry
keeps that API and all existing parser behavior, removing the trailing
semicolon from the four function-forwarding arms in `src/macros.rs`.
These macros must return their parser result in expression position.
The crate denies `semicolon_in_expressions_from_macros` to prevent regression.

As a path dependency, this crate no longer receives Cargo's registry lint
caps. `src/lib.rs` therefore explicitly allows only the external-macro
variant of that lint (from unmodified `nom` 5), plus upstream dead-code and
lifetime-style warnings. The local-macro compatibility lint remains denied.
Remove this patch when upgrading to an IMAP client that uses a newer parser.

Run the upstream parser tests, using the application's lockfile, with:

```sh
cd backend
cargo test --locked -p imap-proto
```
