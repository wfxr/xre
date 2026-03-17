#!/usr/bin/env bash
# install.sh — one-line installer for xre
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/wfxr/xre/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/wfxr/xre/main/install.sh | bash -s -- -v v0.1.0
#   curl -fsSL https://raw.githubusercontent.com/wfxr/xre/main/install.sh | bash -s -- -d /usr/local/bin

set -euo pipefail

REPO="wfxr/xre"
BIN="xre"
TMPDIR_CLEANUP=""

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_tput() { tput "$@" 2>/dev/null || true; }

if [ -t 2 ]; then
    BOLD=$(_tput bold)
    GREEN=$(_tput setaf 2)
    YELLOW=$(_tput setaf 3)
    RED=$(_tput setaf 1)
    RESET=$(_tput sgr0)
else
    BOLD="" GREEN="" YELLOW="" RED="" RESET=""
fi

info()  { printf >&2 '%s[info]%s  %s\n'  "$GREEN"  "$RESET" "$*"; }
warn()  { printf >&2 '%s[warn]%s  %s\n'  "$YELLOW" "$RESET" "$*"; }
error() { printf >&2 '%s[error]%s %s\n'  "$RED"    "$RESET" "$*"; }
die()   { error "$@"; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not found"
}

# ---------------------------------------------------------------------------
# Usage
# ---------------------------------------------------------------------------

usage() {
    cat >&2 <<EOF
${BOLD}install.sh${RESET} — installer for ${BIN}

${BOLD}USAGE${RESET}
    install.sh [OPTIONS]

${BOLD}OPTIONS${RESET}
    -v, --version VERSION   Install a specific version (e.g. v0.1.0)
                            [default: latest release]
    -d, --dir DIR           Installation directory
                            [default: ~/.local/bin or /usr/local/bin for root]
    -h, --help              Show this help message
EOF
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------

VERSION=""
INSTALL_DIR=""

while [ $# -gt 0 ]; do
    case "$1" in
        -v|--version) [ $# -ge 2 ] || die "-v/--version requires an argument"; VERSION="$2"; shift 2 ;;
        -d|--dir)     [ $# -ge 2 ] || die "-d/--dir requires an argument"; INSTALL_DIR="$2"; shift 2 ;;
        -h|--help)    usage; exit 0 ;;
        *)            die "Unknown option: $1 (see --help)" ;;
    esac
done

# ---------------------------------------------------------------------------
# Detect platform
# ---------------------------------------------------------------------------

detect_target() {
    local os arch target
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)  os="linux" ;;
        Darwin) os="darwin" ;;
        *)      die "Unsupported OS: $os" ;;
    esac

    case "$arch" in
        x86_64|amd64)   arch="x86_64" ;;
        aarch64|arm64)  arch="aarch64" ;;
        *)              die "Unsupported architecture: $arch" ;;
    esac

    case "${os}_${arch}" in
        linux_x86_64)   target="x86_64-unknown-linux-musl" ;;
        linux_aarch64)  target="aarch64-unknown-linux-musl" ;;
        darwin_aarch64) target="aarch64-apple-darwin" ;;
        darwin_x86_64)  target="x86_64-apple-darwin" ;;
        *)              die "Unsupported platform: ${os} ${arch}" ;;
    esac

    echo "$target"
}

# ---------------------------------------------------------------------------
# Resolve latest version via GitHub API
# ---------------------------------------------------------------------------

resolve_version() {
    local url header_args response version

    url="https://api.github.com/repos/${REPO}/releases/latest"
    header_args=(-H "Accept: application/vnd.github+json")
    if [ -n "${GITHUB_TOKEN:-}" ]; then
        header_args+=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
    fi

    response=$(curl -fsSL "${header_args[@]}" "$url") \
        || die "Failed to fetch latest release info from GitHub API"

    # Try jq first, fall back to grep
    if command -v jq >/dev/null 2>&1; then
        version=$(echo "$response" | jq -r '.tag_name')
    else
        version=$(echo "$response" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
    fi

    [ -n "$version" ] && [ "$version" != "null" ] \
        || die "Could not determine latest version"

    echo "$version"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
    need curl
    need tar

    local target version install_dir archive_name url archive_dir

    target=$(detect_target)
    info "Detected target: ${BOLD}${target}${RESET}"

    # Resolve version
    if [ -n "$VERSION" ]; then
        version="$VERSION"
    else
        info "Fetching latest release..."
        version=$(resolve_version)
    fi
    info "Version: ${BOLD}${version}${RESET}"

    # Installation directory
    if [ -n "$INSTALL_DIR" ]; then
        install_dir="$INSTALL_DIR"
    elif [ "$(id -u)" -eq 0 ]; then
        install_dir="/usr/local/bin"
    else
        install_dir="${HOME}/.local/bin"
    fi
    mkdir -p "$install_dir"

    # Temporary directory with cleanup trap
    TMPDIR_CLEANUP=$(mktemp -d)
    trap 'rm -rf "$TMPDIR_CLEANUP"' EXIT
    local tmpdir="$TMPDIR_CLEANUP"

    # Download
    archive_name="${BIN}-${version}-${target}.tar.gz"
    url="https://github.com/${REPO}/releases/download/${version}/${archive_name}"
    info "Downloading ${BOLD}${url}${RESET}"
    curl -fSL -o "${tmpdir}/${archive_name}" "$url" \
        || die "Download failed — check that version ${version} exists and has a binary for ${target}"

    # Extract
    tar xzf "${tmpdir}/${archive_name}" -C "$tmpdir"

    # Install (binary is inside a subdirectory matching the archive basename)
    archive_dir="${BIN}-${version}-${target}"
    install -m 755 "${tmpdir}/${archive_dir}/${BIN}" "${install_dir}/${BIN}"
    info "Installed ${BOLD}${BIN}${RESET} to ${BOLD}${install_dir}/${BIN}${RESET}"

    # Verify
    if command -v "${install_dir}/${BIN}" >/dev/null 2>&1; then
        info "$(${install_dir}/${BIN} --version)"
    fi

    # PATH check
    case ":${PATH}:" in
        *":${install_dir}:"*) ;;
        *) warn "${install_dir} is not in your \$PATH — add it to your shell profile" ;;
    esac
}

main
