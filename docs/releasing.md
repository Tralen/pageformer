# Releasing

Pageformer releases are source releases on GitHub. The crates are `publish = false`, so nothing is uploaded to crates.io; the source archive and the signed tag are the release. This guide records the manual process, the candidate checks and the rules that keep the release honest.

## Current setup

- The version source is `Cargo.toml`, under `[workspace.package].version`. The scheme is in the README.
- The canonical history lives in the `operador` bare. The GitHub repository is `Tralen/pageformer` and is the public release target.
- The GitHub repository is `Tralen/pageformer`, configured as `origin` (`git@github.com:Tralen/pageformer.git`). If the remote is missing, add it with:

```sh
git remote add origin git@github.com:Tralen/pageformer.git
```

- The first push established `main`. The tag convention is `v<version>`, such as `v1.0.0`.
- CI is test-only. The workflows are `.github/workflows/ci.yml` and `.github/workflows/gitleaks.yml`; neither publishes anything.

## What may be released

A release must change the library or an executable. Documentation, CI and tooling changes are held and shipped with the next behavior-changing release. Registries and release pages keep every version, and a tag that only updates prose is noise.

The `0.41.4` to `0.41.11` commits were documentation, tooling and test portability work. They were folded into `1.0.0` instead of being tagged separately. The same rule applies to future changes of that kind.

A packaging release is the exception: when the artifact must be reproducible from the tag, the tag has to contain the packaging script, the icon and the version embedded in the binary and in the ODT metadata. `1.0.1` is that case; the engine behavior did not change.

## Releases and assets

A GitHub release is a tag plus a title, notes and assets. The tag fixes the source, while the assets are files attached to the release and can be added or replaced without a new tag:

```sh
gh release upload v<version> dist/pageformer_<version>_<architecture>.deb --clobber
gh release upload v<version> dist/SHA256SUMS --clobber
```

Use that path when the package must be rebuilt from the same tag, for example after a transient builder problem, and update `SHA256SUMS` in the same operation. Do not use it to attach a package built from a commit that is not in the tag: the release would then carry a binary that its source archive cannot reproduce.

A new patch tag is the honest choice when the packaging script, the icon or the documentation that produces the artifact must live in the tag itself. That is why `1.0.1` exists: `packaging/build-deb.sh` and the icon entered after `v1.0.0`, and the version embedded in the binary and in the ODT metadata changed. The engine behavior did not change.

## Candidate checklist

Work from a clean tree and from the canonical history. Do not tag a HEAD behind `operador`.

1. Fetch and fast-forward:

```sh
git fetch --prune operador
git status --short
git merge --ff-only operador/main
```

2. Run the full checks:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features -- --test-threads=1
cargo test --doc
cargo doc --no-deps
markdownlint-cli2
```

3. Run the external validators in [`verification.md`](verification.md): veraPDF on the four PDFs, the LibreOffice conversion for ODT and DOCX, the HTML and EPUB structural checks, and `epubcheck` when it is installed.

4. Scan for secrets and private data:

```sh
gitleaks dir . --redact --no-banner
gitleaks git --redact --no-banner --log-opts="--all"
```

5. Set the version in `Cargo.toml`, update `CHANGELOG.md`, refresh `HANDOFF.md` and regenerate the ODT examples, because `meta:generator` carries the version. Commit with the version in the summary.

6. Verify that the release commit is signed:

```sh
git --no-pager log --show-signature -1
```

7. Confirm that the commit reached the canonical bare:

```sh
git push operador HEAD:refs/heads/main
git fetch operador
git rev-parse HEAD operador/main
```

## Tag and candidate archive

Create the signed tag only after the commit is on `main` and the checks pass:

```sh
git tag -s v<version> -m "Pageformer <version>"
git tag -v v<version>
```

The tag is signed automatically when `tag.gpgSign` is true; `git tag -s` makes the intent explicit. The signing key must already be configured and registered on GitHub.

Build the candidate from the tag. `git archive` includes only tracked files, so the ignored session files never enter the archive:

```sh
mkdir -p dist
git archive --format=tar.gz --prefix=pageformer-<version>/ v<version> > dist/pageformer-<version>.tar.gz
```

Inspect the archive before publishing, from inside `dist`:

```sh
cd dist
tar -tzf pageformer-<version>.tar.gz | head -40
cd ..
```

The checksum file is created after the Debian package is built, so it covers every attachment.

## Debian package

The source archive is the primary release, but Debian and Ubuntu users can install a binary package. `packaging/build-deb.sh` builds it from the checkout:

```sh
git fetch --prune operador
git status --short
git checkout v<version>
packaging/build-deb.sh
```

Build the package from the same signed tag as the source archive, so the package and the archive describe the same code. A packaging change that lands after a tag rides with the next release instead of rebuilding an old one.

The optional argument selects the architecture, which defaults to `dpkg --print-architecture`. The script needs `cargo` and `dpkg-deb`, runs the release build for both workspace binaries and writes `dist/pageformer_<version>_<architecture>.deb`. It installs `pageformer`, `pageformer-publisher`, the desktop entry, the SVG icon and its 256 PNG fallback, the two manual pages, the README, the change log and the license.

The script remaps the Rust build paths, so the binary never carries the builder's home directory, and it derives `SOURCE_DATE_EPOCH` from the last commit. Two runs from the same checkout produce a byte-identical package.

The package declares `libc6 (>= 2.35)` and `libgcc-s1`; it is built against the oldest glibc available in the target matrix, so build it on the oldest supported distribution instead of a newer one. There is no Debian repository: the artifact is a release asset, and the user installs it with `sudo apt install ./pageformer_<version>_<architecture>.deb`. Build each architecture on a machine of that architecture; there is no cross-compilation step.

Verify the package before attaching it:

```sh
shellcheck packaging/build-deb.sh
lintian dist/pageformer_<version>_<architecture>.deb
dpkg-deb --info dist/pageformer_<version>_<architecture>.deb
dpkg-deb --contents dist/pageformer_<version>_<architecture>.deb
dpkg-deb -x dist/pageformer_<version>_<architecture>.deb /tmp/pageformer-deb
/tmp/pageformer-deb/usr/bin/pageformer --version
```

`lintian` must report no errors. The four `embedded-library` errors are overridden with a comment in `/usr/share/lintian/overrides/pageformer`, because `unsafe-libyaml` is a pure Rust port compiled into the binary and the FreeType strings come from the font stack; neither is a system dependency.

Create the checksum file with the source archive and the package together, and verify it before publishing:

```sh
cd dist
sha256sum pageformer-<version>.tar.gz pageformer_<version>_<architecture>.deb > SHA256SUMS
sha256sum -c SHA256SUMS
cd ..
```

Attach the package in the authorized publish step, after the tag and the source archive exist:

```sh
gh release upload v<version> dist/pageformer_<version>_<architecture>.deb
```

CI stays test-only. It may build the package to prove the script works, but it must not upload it without a separate decision.

## Publish

Publishing needs explicit authorization even after the candidate exists. Push the branch and the tag to GitHub, then create the release with the asset basenames, not repository paths:

```sh
git push origin main
git push origin v<version>
gh release create v<version> --verify-tag --generate-notes \
  dist/pageformer-<version>.tar.gz dist/pageformer_<version>_<architecture>.deb dist/SHA256SUMS
```

The release carries the source archive, the Debian binary package and a checksum file. There are no provenance attestations to attach.

Finally, download the assets from the release into a temporary directory and verify the checksum again:

```sh
gh release download v<version> --dir /tmp/pageformer-release
cd /tmp/pageformer-release
sha256sum -c SHA256SUMS
```

## Registry hygiene

- Do not publish a version whose only changes are documentation, CI or tooling.
- Keep the GitHub release and the version source on the same number.
- A GitHub-only release for docs or tooling is not worth a tag, because the source does not change behavior.
- If a crates.io publication is requested later, remove `publish = false`, add the publishing metadata, run `cargo publish --dry-run` and `cargo package`, and publish from a clean checkout with a scoped token kept outside the repository. That is a separate decision and is not part of the current process.

## Privacy

- The Git author e-mail is the GitHub noreply address. Keep it that way.
- Never commit an INI with real values, an SSH key, a token or a machine-specific path. The fictional `assets/examples/confidencial/embargos.ini` is the only INI in the repository and must stay fictional.
- The local and CI secret scans are the backstop, not a substitute for care. If a secret ever reaches a commit, rotate it first, then decide with the maintainer whether the history needs a rewrite.
