# 46: Local signing identity versus release builds

**What to build:** `crates/koharu/tauri.conf.json` (line 44) sets `signingIdentity: "Koharu Local Signing"`, a self-signed certificate that only exists on the user's machine. Builds fail without it, so it cannot ship; but local builds need a stable identity (a changing signature re-prompts for keychain access).

**Decision needed before building.** Options to put to the user with the trade-offs:
- A: keep the identity in the repo for the fork and document that other machines must override it.
- B: take it out of `tauri.conf.json` and supply it per machine (Tauri reads `APPLE_SIGNING_IDENTITY`; a local, uncommitted wrapper script or env), with the committed config unsigned/ad-hoc.
- C: B plus a documented release path (Developer ID plus notarisation) that is a separate build step.
Check how Tauri 2 resolves the identity (env versus config) before recommending. Recommend one; the user picks.

**Blocked by:** user decision

**Status:** needs-triage

- [ ] The user picked an option
- [ ] A local build still works with the stable identity, a clean checkout builds without the certificate
