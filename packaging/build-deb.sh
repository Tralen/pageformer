#!/bin/sh
# Builds the binary Debian package for Pageformer and its converter window.
#
# Usage: packaging/build-deb.sh [architecture]
#
# The architecture defaults to the host dpkg architecture. The script needs
# cargo and dpkg-deb, builds the release binaries and writes
# dist/pageformer_<version>_<architecture>.deb.

set -eu

usage() {
    printf 'usage: %s [architecture]\n' "$0" >&2
    exit 2
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
    printf 'usage: %s [architecture]\n' "$0"
    exit 0
fi
if [ "$#" -gt 1 ]; then
    usage
fi
case "${1:-}" in
    -*) printf 'unknown option: %s\n' "$1" >&2; usage ;;
esac

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

for tool in cargo dpkg-deb; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'missing required tool: %s\n' "$tool" >&2
        exit 1
    fi
done

package=pageformer
version=$(awk -F'"' '
    /^\[workspace\.package\]/ { in_section = 1; next }
    /^\[/ { in_section = 0 }
    in_section && /^version[[:space:]]*=/ { print $2; exit }
' Cargo.toml)

if [ -z "$version" ]; then
    printf 'cannot read the workspace version from Cargo.toml\n' >&2
    exit 1
fi

architecture=${1:-$(dpkg --print-architecture)}

if [ -n "${SOURCE_DATE_EPOCH:-}" ]; then
    epoch=$SOURCE_DATE_EPOCH
else
    epoch=$(git log -1 --format=%ct 2>/dev/null || true)
    if [ -z "$epoch" ]; then
        epoch=$(date +%s)
    fi
fi
export SOURCE_DATE_EPOCH="$epoch"

printf 'Building the release binaries.\n'
# Remap the build paths so the binary never carries this machine's home path.
# The project prefix is listed first because it is nested inside the home one.
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$root=/build/pageformer --remap-path-prefix=$HOME=/build"
cargo build --release --locked -p pageformer -p pageformer-publisher

staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT INT TERM

install -D -m 755 target/release/pageformer "$staging/usr/bin/pageformer"
install -D -m 755 target/release/pageformer-publisher "$staging/usr/bin/pageformer-publisher"
# The SVG is the scalable icon; the 256 PNG is its fallback export.
install -D -m 644 packaging/pageformer-publisher.desktop \
    "$staging/usr/share/applications/pageformer-publisher.desktop"
install -D -m 644 packaging/pageformer.svg \
    "$staging/usr/share/icons/hicolor/scalable/apps/pageformer.svg"
install -D -m 644 packaging/pageformer-256.png \
    "$staging/usr/share/icons/hicolor/256x256/apps/pageformer.png"
install -D -m 644 README.md "$staging/usr/share/doc/$package/README.md"
install -D -m 644 CHANGELOG.md "$staging/usr/share/doc/$package/CHANGELOG.md"
install -D -m 644 LICENSE "$staging/usr/share/doc/$package/LICENSE"
install -D -m 644 LICENSE "$staging/usr/share/doc/$package/copyright"
gzip -9 -n -c CHANGELOG.md > "$staging/usr/share/doc/$package/changelog.gz"
chmod 644 "$staging/usr/share/doc/$package/changelog.gz"
install -D -m 644 packaging/pageformer.1 "$staging/usr/share/man/man1/pageformer.1"
install -D -m 644 packaging/pageformer-publisher.1 \
    "$staging/usr/share/man/man1/pageformer-publisher.1"
gzip -9 -n "$staging/usr/share/man/man1/pageformer.1"
gzip -9 -n "$staging/usr/share/man/man1/pageformer-publisher.1"
install -D -m 644 packaging/lintian-overrides "$staging/usr/share/lintian/overrides/$package"

installed_size=$(du -sk "$staging" | cut -f1)

# The build was made against glibc 2.35, the oldest supported here.
install -d "$staging/DEBIAN"
cat > "$staging/DEBIAN/control" <<EOF
Package: $package
Version: $version
Architecture: $architecture
Installed-Size: $installed_size
Depends: libc6 (>= 2.35), libgcc-s1
Section: text
Priority: optional
Maintainer: Pageformer Maintainers <12503837+Tralen@users.noreply.github.com>
Homepage: https://github.com/Tralen/pageformer
Description: Markdown and YAML to PDF, ODT, DOCX, HTML, EPUB and text
 Pageformer turns Markdown and a single YAML template into six document
 formats from one shared document model and one shared style source.
 .
 It ships the command line and the minimal pageformer-publisher window, and
 it never makes a network request.
EOF

(
    cd "$staging"
    find usr -type f -print0 | sort -z | xargs -0 md5sum
) > "$staging/DEBIAN/md5sums"

find "$staging" -exec touch -h -d "@$epoch" {} +

mkdir -p dist
output="dist/${package}_${version}_${architecture}.deb"
dpkg-deb --build --root-owner-group -Zxz "$staging" "$output"
chmod 644 "$output"

printf 'Wrote %s\n' "$output"
