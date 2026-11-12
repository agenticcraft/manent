#!/usr/bin/env bash
# Build Manent's RPM and publish it into a signed dnf repository (VRT-23, decision-9).
#
# Usage: scripts/release-rpm.sh [--skip-build] [--version X.Y.Z] [--base-url URL] [--publish]
#
#   --skip-build   use the RPM already in src-tauri/target/release/bundle/rpm
#   --version      build this version (default: the one in tauri.conf.json); for test releases
#   --base-url     a test release: serve $RELEASE_HOME/repo-test there yourself, e.g. locally
#   --publish      push the public repository to the gh-pages branch of agenticcraft/manent
#
# Environment:
#   RELEASE_HOME   signing key and repository staging (default ~/.local/share/agenticcraft-release)
#
# Needs createrepo_c and rpm-sign (sudo dnf install createrepo_c rpm-sign). The first run makes
# a signing key "AgenticCraft Releases" in $RELEASE_HOME/gnupg (never in the repository). The
# result in $RELEASE_HOME/repo is static and served by GitHub Pages at $PUBLIC_URL. Users add
# it once:
#   sudo dnf config-manager addrepo --from-repofile=https://agenticcraft.github.io/manent/rpm/manent.repo
#   sudo dnf install manent
# and get updates with dnf or GNOME Software like any other package.
set -euo pipefail

cd "$(dirname "$0")/.."
SKIP_BUILD=0
VERSION=""
PUBLISH=0
PUBLIC_URL="https://agenticcraft.github.io/manent/rpm"
PAGES_REMOTE="git@github.com:agenticcraft/manent.git"
BASE_URL="$PUBLIC_URL"
while [ $# -gt 0 ]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1 ;;
    --version) VERSION="$2"; shift ;;
    --base-url) BASE_URL="${2%/}"; shift ;;
    --publish) PUBLISH=1 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
  shift
done

RELEASE_HOME="${RELEASE_HOME:-$HOME/.local/share/agenticcraft-release}"
export GNUPGHOME="$RELEASE_HOME/gnupg"
# Test releases never mix with what is public.
REPO="$RELEASE_HOME/repo"
[ "$BASE_URL" = "$PUBLIC_URL" ] || REPO="$RELEASE_HOME/repo-test"
if [ "$PUBLISH" = 1 ] && { [ "$BASE_URL" != "$PUBLIC_URL" ] || [ -n "$VERSION" ]; }; then
  echo "--publish takes the version in tauri.conf.json and the public URL" >&2
  exit 2
fi
KEY_NAME="AgenticCraft Releases <releases@agenticcraft.ai>"
ARCH="$(uname -m)"

for tool in createrepo_c rpmsign gpg; do
  command -v "$tool" >/dev/null || { echo "missing $tool: sudo dnf install createrepo_c rpm-sign" >&2; exit 1; }
done

# The signing key (made once, kept outside the repository).
mkdir -p "$GNUPGHOME" && chmod 700 "$GNUPGHOME"
if ! gpg --batch --list-secret-keys "$KEY_NAME" >/dev/null 2>&1; then
  echo "making the signing key in $GNUPGHOME"
  gpg --batch --passphrase '' --quick-gen-key "$KEY_NAME" rsa4096 sign 5y
fi
FPR="$(gpg --batch --with-colons --list-secret-keys "$KEY_NAME" | awk -F: '/^fpr/ {print $10; exit}')"

# The RPM. No paths of this computer in the binaries (panic locations of dependencies).
if [ "$SKIP_BUILD" = 0 ]; then
  export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$HOME/.cargo=/cargo --remap-path-prefix=$PWD=/manent"
  export CFLAGS="${CFLAGS:-} -ffile-prefix-map=$HOME/.cargo=/cargo -ffile-prefix-map=$PWD=/manent"
  export CXXFLAGS="${CXXFLAGS:-} -ffile-prefix-map=$HOME/.cargo=/cargo -ffile-prefix-map=$PWD=/manent"
  if [ -n "$VERSION" ]; then
    npm run tauri build -- --config "{\"version\": \"$VERSION\"}"
  else
    npm run tauri build
  fi
fi
V="${VERSION:-$(python3 -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["version"])')}"
RPM="src-tauri/target/release/bundle/rpm/Manent-$V-1.$ARCH.rpm"
[ -f "$RPM" ] || { echo "no $RPM" >&2; exit 1; }

mkdir -p "$REPO/fedora/$ARCH"
cp "$RPM" "$REPO/fedora/$ARCH/manent-$V-1.$ARCH.rpm"
rpmsign --addsign --define "_gpg_name $FPR" --define "_gpg_path $GNUPGHOME" \
  "$REPO/fedora/$ARCH/manent-$V-1.$ARCH.rpm"

# Repository metadata, signed too (repo_gpgcheck).
createrepo_c --quiet --update "$REPO/fedora/$ARCH"
rm -f "$REPO/fedora/$ARCH/repodata/repomd.xml.asc"
gpg --batch --yes --local-user "$FPR" --armor --detach-sign "$REPO/fedora/$ARCH/repodata/repomd.xml"
gpg --batch --armor --export "$FPR" > "$REPO/RPM-GPG-KEY-agenticcraft"
cat > "$REPO/manent.repo" <<EOF
[manent]
name=Manent by AgenticCraft
baseurl=$BASE_URL/fedora/\$basearch
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=$BASE_URL/RPM-GPG-KEY-agenticcraft
EOF

echo "released manent $V into $REPO (key $FPR); serve it at $BASE_URL"
ls -1 "$REPO/fedora/$ARCH"/*.rpm

# The public repository: everything in $REPO under rpm/ on the gh-pages branch.
if [ "$PUBLISH" = 1 ]; then
  NAME="$(git config user.name)"
  EMAIL="$(git config user.email)"
  PAGES="$(mktemp -d)"
  trap 'rm -rf "$PAGES"' EXIT
  if git ls-remote --exit-code --heads "$PAGES_REMOTE" gh-pages >/dev/null; then
    git clone --quiet --depth 1 --branch gh-pages "$PAGES_REMOTE" "$PAGES"
  else
    git -C "$PAGES" init --quiet --initial-branch gh-pages
    git -C "$PAGES" remote add origin "$PAGES_REMOTE"
  fi
  mkdir -p "$PAGES/rpm"
  rsync -a --delete "$REPO/" "$PAGES/rpm/"
  touch "$PAGES/.nojekyll"
  cp scripts/pages-index.html "$PAGES/index.html"
  if find "$PAGES" -path "$PAGES/.git" -prune -o \( -name '*.gpg' -o -name '*.key' -o -name 'private-keys*' -o -name 'trustdb*' \) -print | grep -q .; then
    echo "refusing to publish: key material in $PAGES" >&2
    exit 1
  fi
  git -C "$PAGES" add -A
  if git -C "$PAGES" diff --cached --quiet; then
    echo "nothing new to publish"
  else
    git -C "$PAGES" -c user.name="$NAME" -c user.email="$EMAIL" commit --quiet -m "Manent $V"
    git -C "$PAGES" push --quiet origin gh-pages
    echo "pushed manent $V to gh-pages; live at $PUBLIC_URL in a few minutes"
  fi
fi
