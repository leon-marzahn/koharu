# Worker rules (every card)

- Work in the git worktree you were given. Do NOT commit, push, or switch branches.
- Write failing vitest tests first (and Rust tests for backend changes), then implement. Tests must use real backend identifiers and go through the real edit paths (pipeline authorship check, canvas commit/cancel path). A gesture that changes nothing must cancel the canvas edit. Model output may contain null where the schema says string.
- Touch only the files the ticket needs.
- Do NOT run `bun run format` or `bun run check` (63 pre-existing unformatted files). Format only touched files with `bunx oxfmt <file>`; oxfmt can rewrite unrelated lines (it does in CanvasWorkspace.tsx), so check `git diff --stat` and revert unrelated hunks.
- Run `bun install --frozen-lockfile` once in the worktree. Then `bun run test`, `bun run lint`, and `bunx tsc --noEmit` (from packages/koharu). The unused `clamp` lint warning in ToolBar.tsx is pre-existing.
- Every new UI string goes into EVERY locale file in `packages/koharu/public/locales` (en-US, es-ES, ja-JP, ko-KR, pt-BR, ru-RU, tr-TR, zh-CN, zh-TW).
- Alt is used by the canvas; do not rebind it.
- Any new config field needs `#[serde(default)]` plus a test that loads a config lacking it. Any new component stored in a project needs a revision, `#[serde(default)]`, and a load test for old projects. A component the pipeline writes needs `SHARED_WITH_PIPELINE`; user-only components must not have it.
- Rust: `export CARGO_TARGET_DIR=/Users/lmarzahn/Development/open-source/koharu/target` (shared target). After Rust type changes: touch `crates/koharu-scene/src/lib.rs`, `crates/koharu-app/src/lib.rs` and `crates/koharu-app/src/bin/generate.rs`, run `cargo run -q -p koharu-app --bin generate`, and check the `packages/bridge/src/protocol.ts` diff. Run `cargo test -p koharu-scene -p koharu-translator -p koharu-pipeline -p koharu-app --lib` and `cargo fmt` on touched crates.
- Tick the ticket's checkboxes and set its Status to done when finished. End with a short summary of what changed and how it was verified.
