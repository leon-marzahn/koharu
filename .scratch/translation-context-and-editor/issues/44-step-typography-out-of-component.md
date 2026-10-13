# 44: Move `stepTypography` out of `CanvasWorkspace.tsx`

**What to build:** `packages/koharu/components/editor/CanvasWorkspace.tsx` (line 91) exports `stepTypography` next to a component. That breaks fast refresh for the file and puts pure logic in a component module. Move it to the lib module that already owns typography or font helpers (find it; add a file only if none fits), update the importers and its tests, and delete the export from the component file.
- No behaviour change.

**Blocked by:** none

**Status:** done

- [x] `stepTypography` lives outside component files; every importer updated
- [x] `bun run test`, `bun run lint` and `bunx tsc --noEmit` in `packages/koharu` pass
