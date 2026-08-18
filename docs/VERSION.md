# Versioning & releasing

## Release a new version

A release is its own PR — never bundled with feature work, never pushed straight
to `main`. Bundled, the repo's version drifts from what was actually published:
if the bump merges and nobody tags, the next feature bumps again and the skipped
version never exists as a build.

Pick the new version (e.g. `0.1.1`) and set it in **three files** — they must all match:

1. `Cargo.toml` → `[workspace.package].version`
2. `apps/desktop/src-tauri/tauri.conf.json` → `version`
3. `apps/desktop/package.json` → `version`

```bash
git checkout -b release/v0.1.1
# set the three files, then
git commit -am "chore: release v0.1.1"
gh pr create --title "chore: release v0.1.1"
```

Merge once CI is green, then tag `main` — the tag is what triggers the release:

```bash
git checkout main && git pull
git tag v0.1.1 && git push origin v0.1.1
```

Pushing the `v*` tag runs `.github/workflows/release.yml`, which builds, signs,
and publishes the bundles + `latest.json`.

> **The tag must match `tauri.conf.json`.** The app is stamped with the
> `tauri.conf.json` version, and the updater compares it to `latest.json` (whose
> version comes from the tag). If they differ, the updater breaks — it either
> offers an update the user already has, or misses real ones.

## Picking the number

Pre-1.0 the minor is the only signal users get, and they see it in the update
prompt: bump it for anything they will notice — features, layout or behaviour
changes, migrations. Reserve the patch for fixes to an already-released version.

## Why three files?

Pure Rust apps keep one version (`Cargo.toml`). The extra copies come from Tauri
(`tauri.conf.json`) and npm (`package.json`) each needing their own. To collapse
this later, have CI derive the version from the tag (`${TAG#v}`) at build time —
that makes the tag the single source of truth and enforces the match above.
