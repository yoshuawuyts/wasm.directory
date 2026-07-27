#!/bin/sh
# install.sh — Download and install the wasm(1) CLI tool.
#
# Usage:
#   curl --proto '=https' --tlsv1.2 -LsSf https://github.com/yoshuawuyts/wasm.directory/releases/latest/download/install.sh | sh
#
# Options:
#   --version <VERSION>   Install a specific version (e.g. 0.3.0)
#
# Environment variables:
#   WASM_VERSION          Same as --version
#   CARGO_HOME            Override install directory (default: $HOME/.cargo)

set -eu

REPO="yoshuawuyts/wasm.directory"
BINARY_NAME="component"

# --- helpers ----------------------------------------------------------------

say() {
    printf '%s\n' "$*"
}

err() {
    say "error: $*" >&2
    exit 1
}

need() {
    if ! command -v "$1" > /dev/null 2>&1; then
        err "need '$1' (command not found)"
    fi
}

# --- detect platform --------------------------------------------------------

detect_target() {
    local _os _arch _target

    _os="$(uname -s)"
    _arch="$(uname -m)"

    case "$_os" in
        Linux)
            case "$_arch" in
                x86_64)  _target="x86_64-unknown-linux-gnu" ;;
                aarch64) _target="aarch64-unknown-linux-gnu" ;;
                *)       err "unsupported architecture: $_arch on $_os" ;;
            esac
            ;;
        Darwin)
            case "$_arch" in
                x86_64)  _target="x86_64-apple-darwin" ;;
                arm64)   _target="aarch64-apple-darwin" ;;
                *)       err "unsupported architecture: $_arch on $_os" ;;
            esac
            ;;
        *)
            err "unsupported operating system: $_os (use install.ps1 for Windows)"
            ;;
    esac

    echo "$_target"
}

# --- resolve version --------------------------------------------------------

resolve_version() {
    local _version _url _redirect

    _version="${1:-latest}"

    if [ "$_version" = "latest" ]; then
        _url="https://github.com/${REPO}/releases/latest"
        # Follow the redirect and extract the tag from the final URL
        if command -v curl > /dev/null 2>&1; then
            _redirect="$(curl --proto '=https' --tlsv1.2 -Ls -o /dev/null -w '%{url_effective}' "$_url")"
        elif command -v wget > /dev/null 2>&1; then
            _redirect="$(wget -q -O /dev/null --server-response "$_url" 2>&1 | grep -i 'Location:' | tail -1 | awk '{print $2}' | tr -d '\r')"
        else
            err "need 'curl' or 'wget' to resolve latest version"
        fi

        # Extract version from URL like .../releases/tag/v0.3.0
        _version="$(echo "$_redirect" | sed 's|.*/tag/||')"

        if [ -z "$_version" ]; then
            err "could not resolve latest version from GitHub"
        fi
    fi

    echo "$_version"
}

# --- download ---------------------------------------------------------------

download() {
    local _url _output
    _url="$1"
    _output="$2"

    if command -v curl > /dev/null 2>&1; then
        curl --proto '=https' --tlsv1.2 -fL "$_url" -o "$_output"
    elif command -v wget > /dev/null 2>&1; then
        wget -q "$_url" -O "$_output"
    else
        err "need 'curl' or 'wget' to download files"
    fi
}

# --- main -------------------------------------------------------------------

main() {
    local _version _target _install_dir _archive_url _tmpdir

    # Parse arguments
    _version="${WASM_VERSION:-latest}"
    while [ $# -gt 0 ]; do
        case "$1" in
            --version)
                shift
                _version="$1"
                ;;
            *)
                err "unknown argument: $1"
                ;;
        esac
        shift
    done

    need uname
    need tar

    _target="$(detect_target)"
    _version="$(resolve_version "$_version")"

    _install_dir="${CARGO_HOME:-$HOME/.cargo}/bin"
    _archive_url="https://github.com/${REPO}/releases/download/${_version}/${BINARY_NAME}-${_target}.tar.gz"

    say "Installing ${BINARY_NAME} v${_version} (${_target})"
    say "  from: ${_archive_url}"
    say "  to:   ${_install_dir}/${BINARY_NAME}"
    say ""

    # Create a temporary directory for the download
    _tmpdir="$(mktemp -d)"
    trap 'rm -rf "$_tmpdir"' EXIT

    # Download and extract
    say "Downloading..."
    download "$_archive_url" "$_tmpdir/archive.tar.gz"

    say "Extracting..."
    tar xzf "$_tmpdir/archive.tar.gz" -C "$_tmpdir"

    # Install the binary
    mkdir -p "$_install_dir"
    mv "$_tmpdir/${BINARY_NAME}" "$_install_dir/${BINARY_NAME}"
    chmod +x "$_install_dir/${BINARY_NAME}"

    say ""
    say "Installed ${BINARY_NAME} to ${_install_dir}/${BINARY_NAME}"

    # Check if the install directory is on PATH
    case ":${PATH}:" in
        *":${_install_dir}:"*)
            say ""
            say "Run '${BINARY_NAME} --version' to verify the installation."
            ;;
        *)
            say ""
            say "warning: ${_install_dir} is not in your PATH"
            say ""
            say "Add it by running one of:"
            say "  echo 'export PATH=\"${_install_dir}:\$PATH\"' >> ~/.bashrc"
            say "  echo 'export PATH=\"${_install_dir}:\$PATH\"' >> ~/.zshrc"
            ;;
    esac
}

main "$@"
