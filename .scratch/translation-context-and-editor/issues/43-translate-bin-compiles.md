# 43: `koharu-translator` translate bin compiles

**What to build:** `crates/koharu-translator/src/bin/translate.rs` does not compile: `serde_json::to_string_pretty(&translated)` (line 98) needs `Translated: Serialize`, which it lacks (`cargo check -p koharu-translator --bin translate`). It is a normal workspace bin ("Run Koharu's configured translator", a CLI over the crate's public API), not dead code, so fix it rather than delete it.
- Smallest fix that keeps the crate's public types honest: derive `Serialize` on `Translated` if its fields are plain, otherwise serialise a small local struct in the bin. Pick the first unless it drags a dependency into the library.
- Run `cargo check -p koharu-translator --bin translate` once; it must be clean. Do not run the bin against a real provider.

**Blocked by:** none

**Status:** done

- [x] `cargo check -p koharu-translator --bin translate` passes
- [x] No new dependency
