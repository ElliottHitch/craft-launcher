# Maintain your Craft App Suite forks

This checkout targets public releases under `ElliottHitch`. The app catalog is the source of
repository names. Each personal repository keeps `storytold/<same repository name>` as `upstream`.
GitHub forks keep the shared history, so you can merge selected upstream updates while preserving
your own commits. You still need to resolve conflicts and review upstream changes.

## Setup status on 2026-10-23

The launcher and nine app forks are published. SoundCraft, DeckCraft, CADCraft, and GridCraft
are prepared locally under `repos/`, but GitHub is rejecting creation of their personal forks
with `was submitted too quickly`, including through the authenticated CLI. Their personal
release workflows and update links are ready to publish after that restriction clears.

All thirteen local app checkouts have the personal `origin` and original `upstream` remotes.
No personal binary releases have been built or published yet.

## Prepare local checkouts

From the ArtCraft Launcher checkout, run:

```powershell
cargo xtask forks list
cargo xtask forks clone
cargo xtask forks status
```

`clone` requires each public GitHub fork to exist. It leaves the launcher in this workspace and
clones apps into `repos/<repo>` with full commit history. It adds `upstream` when that remote is
missing. Existing checkouts must have the expected GitHub remotes and full history. The command
refuses a mismatch instead of changing a remote or replacing files. Uncommitted changes stay in
place because these commands do not check out commits or merge branches.

Pass one app id or repository name to limit any command:

```powershell
cargo xtask forks clone photocraft
cargo xtask forks status craft-launcher
cargo xtask forks fetch photocraft
```

`fetch` updates only `refs/remotes/upstream/*`. It leaves your branches and tags in place.
No `forks` command merges, rebases, resets, pushes, or publishes a release.

## Merge a selected upstream update

Commit your personal changes before merging. For PhotoCraft, use a review branch:

```powershell
cargo xtask forks fetch photocraft
Set-Location repos/photocraft
git switch main
git switch -c review/upstream-2026-10-23
git log --oneline main..upstream/main
git diff --stat main...upstream/main
git merge --no-ff --no-commit upstream/main
```

Replace `upstream/main` with the upstream commit you chose. Inspect the staged merge and resolve
conflicts without discarding your personal patches. Use `git merge --abort` if you decide to
reject the update. Commit the accepted merge and open a pull request against your fork's `main`.
Merge that pull request only after review.

Record the full upstream commit SHA in the pull request and release notes. Keep that record
separate from the package version. A later merge uses the shared history to distinguish your
patches from upstream work.

## Review before releasing

Review both the incoming upstream diff and the final merge, especially network access, file
operations, process execution, install scripts, dependencies, and workflow permissions. Check
that the resulting app still includes the personal patches you intend to keep.

Ask Codex to review the app and patch the findings you accept. Run dependency scans such as
`cargo audit` when you request them, separately from packaging. A dependency scanner checks
known advisories; it does not replace source review.

Inherited CI, external security-service, and release workflows are disabled in the configured
personal forks. Their source files remain available. Only the personal manual Windows build
is enabled; it runs no automated tests or security scans. The `review-note` input records your
own review of the chosen source commit.

Keep publishing manual. Do not enable automatic upstream merges or publish packages directly
from an upstream update notification.

## Build and publish a personal Windows release

The [personal Windows workflow](../.github/workflows/personal-windows.yml) builds the launcher.
The [caller template](templates/personal-release.yml) connects an app fork to that workflow.
The installed callers pin the reusable workflow to a full commit SHA. Update that pin only
after reviewing changes to the build workflow.
An app may need its own Windows build dependencies or packaging adjustments before its first
personal release. The workflow accepts an optional reviewed
`packaging/windows/personal-prepare.ps1` in that app for those dependencies.

Use a numeric `major.minor.patch` version that is greater than the installed version and every
earlier personal release of that app. Set it in the app's workspace `Cargo.toml`. For the
launcher, use `cargo xtask version set X.Y.Z`. Keep the manifest version, release tag, and asset
version consistent. SemVer build metadata such as `+elliott.1` does not make a version newer,
and Windows MSI upgrades also need an increasing numeric version. The workflow limits major
and minor to 255 and patch to 65535, and rejects a version at or below an existing numeric tag
or release, including inherited upstream tags.

Run the workflow with `app-id`, the full 40-character `source-sha`, `version`, and `review-note`.
The selected commit must belong to the caller fork, and its workspace version must match the
requested version. The workflow creates a draft release. Inspect the draft's source commit,
version, packages, and `SHA256SUMS.txt`, then publish it explicitly as the latest release in GitHub. Drafts are not offered as launcher
updates. The initial workflow builds unsigned Windows packages. Other platforms need their
own release setup.

The Windows launcher requires matching Authenticode signatures for its own self-update.
Unsigned personal launcher builds therefore need manual reinstallation. Installing and updating
the suite's app packages is a separate path. Keep the self-update signature check intact; add
your own consistent signing setup before enabling unattended launcher replacement.

The launcher installs and updates only from the catalog's `ElliottHitch` releases. Until a
personal release is published with the expected asset names and checksums, that app has no
personal download available. The official release feed is disabled for this configuration.

Personal packages retain the existing app names, bundle ids, install paths, and settings
locations. They replace the standard app installation and share its settings. They are not
separate side-by-side installations.

## Use the local mirror

To build the app checkouts in `repos` without pulling changes, run from the launcher root:

```powershell
cargo xtask local sync --root repos --no-pull --only photocraft
cargo xtask local serve --mirror repos/local-mirror
```

The local feed uses each app's catalog repository, so its release metadata matches the personal
download policy. Enable the existing `local-mirror` feature and loopback URL when running the
launcher against that mirror. The local mirror command uses the app's existing packaging
scripts and may need a supported build environment for that app.
