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

3. Run the external validators in [`verification.md`](verification.md): veraPDF on the three PDFs, the LibreOffice conversion for ODT and DOCX, the HTML and EPUB structural checks, and `epubcheck` when it is installed.

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
cd dist
sha256sum pageformer-<version>.tar.gz > SHA256SUMS
```

Inspect the archive before publishing, from inside `dist`:

```sh
tar -tzf pageformer-<version>.tar.gz | head -40
sha256sum -c SHA256SUMS
cd ..
```

## Publish

Publishing needs explicit authorization even after the candidate exists. Push the branch and the tag to GitHub, then create the release with the asset basenames, not repository paths:

```sh
git push origin main
git push origin v<version>
gh release create v<version> --verify-tag --generate-notes \
  dist/pageformer-<version>.tar.gz dist/SHA256SUMS
```

The project ships no binaries, so there are no provenance attestations to attach. The source archive and its checksum are the artifacts.

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
