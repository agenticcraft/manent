#!/usr/bin/env bash
# Build Manent's RPM and publish it into a signed dnf repository (VRT-23, decision-9).
#
# Usage: scripts/release-rpm.sh [--skip-build] [--version X.Y.Z] [--base-url URL]
#
#   --skip-build   use the RPM already in src-tauri/target/release/bundle/rpm
#   --version      build this version (default: the one in tauri.conf.json); for test releases
#   --base-url     where the repository will be served (default https://agenticcraft.ai/manent/rpm)
#
# Environment:
#   RELEASE_HOME   signing key and repository staging (default ~/.local/share/agenticcraft-release)
#
# Needs createrepo_c and rpm-sign (sudo dnf install createrepo_c rpm-sign). The first run makes
# a signing key "AgenticCraft Releases" in $RELEASE_HOME/gnupg (never in the repository). The
# result in $RELEASE_HOME/repo is static: upload it as-is to <base-url>/. Users add it once:
#   sudo dnf config-manager addrepo --from-repofile=<base-url>/manent.repo
#   sudo dnf install manent
# and get updates with dnf or GNOME Software like any other package.
set -euo pipefail

cd "$(dirname "$0")/.."
SKIP_BUILD=0
VERSION=""
BASE_URL="https://agenticcraft.ai/manent/rpm"
while [ $# -gt 0 ]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1 ;;
    --version) VERSION="$2"; shift ;;
    --base-url) BASE_URL="${2%/}"; shift ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
  shift
done

RELEASE_HOME="${RELEASE_HOME:-$HOME/.local/share/agenticcraft-release}"
export GNUPGHOME="$RELEASE_HOME/gnupg"
REPO="$RELEASE_HOME/repo"
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

# The RPM.
if [ "$SKIP_BUILD" = 0 ]; then
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

echo "published manent $V in $REPO (key $FPR); serve it at $BASE_URL"
ls -1 "$REPO/fedora/$ARCH"/*.rpm
