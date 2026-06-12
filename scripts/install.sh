#!/bin/sh
set -eu

REPO="${HM_INSTALL_REPO:-INONONO66/harness-manager}"
VERSION="${HM_INSTALL_VERSION:-latest}"
PREFIX="${HM_INSTALL_PREFIX:-$HOME/.local}"
BIN_DIR="${HM_INSTALL_BIN_DIR:-}"
RUN_INIT=0
RUN_HARNESS_INSTALL=0
DRY_RUN="${HM_INSTALL_DRY_RUN:-0}"

usage() {
    cat <<'USAGE'
Install hm from GitHub release artifacts.

Usage:
  install.sh [--version <tag>] [--prefix <dir>] [--bin-dir <dir>] [--init] [--install-harnesses] [--dry-run]

Options:
  --version <tag>        Release tag to install, for example v0.2.8. Defaults to latest.
  --prefix <dir>         Installation prefix. Defaults to ~/.local.
  --bin-dir <dir>        Exact binary directory. Defaults to <prefix>/bin.
  --init                 Run hm init after installing the binary.
  --install-harnesses    Run hm init --install after installing the binary.
  --dry-run              Print the selected artifact and destination without downloading.
  -h, --help             Show this help.

Environment:
  HM_INSTALL_REPO        GitHub repo owner/name. Defaults to INONONO66/harness-manager.
  HM_INSTALL_VERSION     Release tag or latest.
  HM_INSTALL_PREFIX      Installation prefix.
  HM_INSTALL_BIN_DIR     Exact binary directory.
  HM_INSTALL_DRY_RUN     Set to 1 for dry-run.
USAGE
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            [ "$#" -ge 2 ] || {
                echo "error: --version requires a tag" >&2
                exit 2
            }
            VERSION="$2"
            shift 2
            ;;
        --prefix)
            [ "$#" -ge 2 ] || {
                echo "error: --prefix requires a directory" >&2
                exit 2
            }
            PREFIX="$2"
            shift 2
            ;;
        --bin-dir)
            [ "$#" -ge 2 ] || {
                echo "error: --bin-dir requires a directory" >&2
                exit 2
            }
            BIN_DIR="$2"
            shift 2
            ;;
        --init)
            RUN_INIT=1
            shift
            ;;
        --install-harnesses)
            RUN_INIT=1
            RUN_HARNESS_INSTALL=1
            shift
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "error: unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [ -z "$BIN_DIR" ]; then
    BIN_DIR="$PREFIX/bin"
fi

case "$(uname -s)" in
    Linux) os="linux" ;;
    Darwin) os="darwin" ;;
    *)
        echo "error: unsupported OS: $(uname -s)" >&2
        exit 1
        ;;
esac

case "$(uname -m)" in
    x86_64 | amd64) arch="x86_64" ;;
    arm64 | aarch64) arch="aarch64" ;;
    *)
        echo "error: unsupported architecture: $(uname -m)" >&2
        exit 1
        ;;
esac

asset="hm-$arch-$os.tar.gz"
if [ "$VERSION" = "latest" ]; then
    base_url="https://github.com/$REPO/releases/latest/download"
else
    base_url="https://github.com/$REPO/releases/download/$VERSION"
fi
url="$base_url/$asset"
checksum_url="$url.sha256"

echo "hm installer"
echo "  repo:        $REPO"
echo "  version:     $VERSION"
echo "  artifact:    $asset"
echo "  destination: $BIN_DIR/hm"

if [ "$DRY_RUN" = "1" ]; then
    echo "dry-run: would download $url"
    exit 0
fi

tmp_dir="$(mktemp -d)"
cleanup() {
    rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

archive="$tmp_dir/$asset"
checksum_file="$tmp_dir/$asset.sha256"

if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$archive"
    curl -fsSL "$checksum_url" -o "$checksum_file"
elif command -v wget >/dev/null 2>&1; then
    wget -q "$url" -O "$archive"
    wget -q "$checksum_url" -O "$checksum_file"
else
    echo "error: install requires curl or wget" >&2
    exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
    (cd "$tmp_dir" && sha256sum -c "$asset.sha256")
elif command -v shasum >/dev/null 2>&1; then
    expected="$(awk '{print $1}' "$checksum_file")"
    actual="$(shasum -a 256 "$archive" | awk '{print $1}')"
    if [ "$expected" != "$actual" ]; then
        echo "error: checksum mismatch for $asset" >&2
        exit 1
    fi
else
    echo "warning: sha256sum or shasum not found; skipping checksum verification" >&2
fi

tar -xzf "$archive" -C "$tmp_dir"
mkdir -p "$BIN_DIR"
if command -v install >/dev/null 2>&1; then
    install -m 0755 "$tmp_dir/hm" "$BIN_DIR/hm"
else
    cp "$tmp_dir/hm" "$BIN_DIR/hm"
    chmod 0755 "$BIN_DIR/hm"
fi

echo "installed: $BIN_DIR/hm"

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "note: add $BIN_DIR to PATH to run hm from any shell" ;;
esac

if [ "$RUN_INIT" = "1" ]; then
    if [ "$RUN_HARNESS_INSTALL" = "1" ]; then
        "$BIN_DIR/hm" init --install
    else
        "$BIN_DIR/hm" init
    fi
fi
