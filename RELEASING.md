# Releasing

The single source of truth for how a release of `ctddump` is cut. `README.md`
and `CLAUDE.md` only point here.

## Branching model

- **`main`**: stable, released code. Every commit on `main` is a release merge.
- **`develop`**: integration branch; day-to-day work lands here.
- Multi-session features use `git-flow` (AVH Edition):
  `git flow feature start/finish <name>`.
- Releases are **not** cut with `git flow release`. `develop` is merged into
  `main` manually and tagged (see below), so each release appears on `main`'s
  first-parent history as `Merge develop into main for vX.Y.Z`.

## Versioning

Follow [Semantic Versioning](https://semver.org/): `MAJOR.MINOR.PATCH`.

- **PATCH** (`0.4.1` → `0.4.2`): bug fixes, docs, licensing; no behaviour change.
- **MINOR** (`0.3.0` → `0.4.0`): new, backward-compatible features or flags.
- **MAJOR**: incompatible changes (output schema, CLI, config format).

The version lives in `Cargo.toml` (`[package] version`); keep `Cargo.lock` in sync.

## Changelog

`CHANGELOG.md` follows [Keep a Changelog](https://keepachangelog.com/). While
developing, add entries under `## [Unreleased]` in the relevant group
(`Added` / `Changed` / `Deprecated` / `Removed` / `Fixed` / `Security`). At
release time those entries move into the new version's section.

## Release steps

Start from a clean `develop` with everything merged and `cargo test` passing.
Replace `X.Y.Z` with the new version and `PREV` with the previous tag.

1. **Bump the version** in `Cargo.toml`, then sync the lockfile:
   ```bash
   # edit Cargo.toml: version = "X.Y.Z"
   cargo update -p ctddump --precise X.Y.Z
   ```

2. **Update `CHANGELOG.md`:**
   - Move the `## [Unreleased]` entries into a new `## [X.Y.Z] - YYYY-MM-DD`
     section, leaving a fresh empty `## [Unreleased]` above it.
   - Refresh the compare links at the bottom:
     ```
     [Unreleased]: https://github.com/AIQC-Hub/ctddump/compare/vX.Y.Z...HEAD
     [X.Y.Z]: https://github.com/AIQC-Hub/ctddump/compare/vPREV...vX.Y.Z
     ```

3. **Commit the bump** on `develop`:
   ```bash
   git add Cargo.toml Cargo.lock CHANGELOG.md
   git commit -m "Bump version to vX.Y.Z"
   ```

4. **Merge into `main` and tag:**
   ```bash
   git checkout main
   git merge --no-ff develop -m "Merge develop into main for vX.Y.Z"
   git tag -a vX.Y.Z -m "Release vX.Y.Z"
   ```

5. **Push everything.** Pushing the tag publishes to crates.io, so make sure the
   preceding steps are right before this point:
   ```bash
   git push origin main
   git push origin vX.Y.Z
   git checkout develop
   git push origin develop
   ```

> Tags are `v`-prefixed (`vX.Y.Z`) even though git-flow's version-tag prefix is
> empty; apply the tag manually as shown.

## What the push triggers

On push to `main`:

- **CI** (`.github/workflows/ci.yml`) runs the test suite on every push/PR to `main`.
- **Docs** (`.github/workflows/pages.yml`) rebuilds the mdBook site and deploys
  it to GitHub Pages when the push touches `docs/**`.

On push of a `vX.Y.Z` tag:

- **Publish** (`.github/workflows/publish.yml`) runs the test suite, then uploads
  the crate to [crates.io](https://crates.io/crates/ctddump).

Confirm they are green:

```bash
gh run list --branch main --limit 3
gh run list --workflow publish.yml --limit 1
```

### Publishing to crates.io

The upload is automatic, so **step 5 is the point of no return**: a published
version can be yanked but never replaced or re-uploaded. Before pushing the tag,
check that `Cargo.toml`, `CHANGELOG.md`, and the tag all name the same version.

Three gates protect the upload:

- the test suite runs first (a tag push is no proof CI ran on that commit);
- the job fails if the tag disagrees with the `Cargo.toml` version;
- `cargo publish --dry-run --locked` builds the real tarball before uploading.

Authentication uses crates.io Trusted Publishing, so this repository stores no
API token. It relies on a one-time setup that must stay in sync:

| Where | Setting |
|-------|---------|
| crates.io → crate Settings → Trusted Publishing | owner `AIQC-Hub`, repository `ctddump`, workflow `publish.yml`, environment `crates-io` |
| GitHub → Settings → Environments | an environment named `crates-io`, holding no secrets or variables |

Renaming the workflow file, or changing the environment on one side only, breaks
authentication with an error that does not obviously point at the cause.

To rehearse without publishing, run the workflow manually from the Actions tab:
a `workflow_dispatch` run defaults to a dry run, which packages and verifies but
uploads nothing.

## Hotfixes

For an urgent fix on top of a release: branch from `main`
(`git flow hotfix start X.Y.Z`), make the fix, bump the PATCH version and
changelog, then merge back into **both** `main` and `develop` and tag `vX.Y.Z`.
