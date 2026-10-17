#!/usr/bin/env bash
#
# Build script for firebird-5-mcp (Rust)
#
# Usage:
#   ./scripts/build.sh              # Build for current platform
#   ./scripts/build.sh linux        # Build for linux/amd64 + linux/arm64
#   ./scripts/build.sh darwin       # Build for darwin/amd64 + darwin/arm64
#   ./scripts/build.sh windows      # Build for windows/amd64 + windows/arm64
#   ./scripts/build.sh all          # Build for all platforms
#
set -euo pipefail

# Resolve project root (parent of scripts/)
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${ROOT}/bin"
DIST_DIR="${ROOT}/dist"

# Supported platforms: "os/arch"
PLATFORMS=(
  "linux/amd64"
  "linux/arm64"
  "darwin/amd64"
  "darwin/arm64"
  "windows/amd64"
  "windows/arm64"
)

# Map short names to platform lists
resolve_platforms() {
  case "${1:-current}" in
    linux)
      echo "linux/amd64 linux/arm64"
      ;;
    darwin|macos)
      echo "darwin/amd64 darwin/arm64"
      ;;
    windows)
      echo "windows/amd64 windows/arm64"
      ;;
    all)
      echo "${PLATFORMS[*]}"
      ;;
    current)
      # Detect current platform
      local os arch
      os="$(uname -s | tr '[:upper:]' '[:lower:]')"
      arch="$(uname -m)"
      case "${arch}" in
        x86_64|amd64) arch="amd64" ;;
        aarch64|arm64) arch="arm64" ;;
      esac
      echo "${os}/${arch}"
      ;;
    *)
      # Assume it's already in "os/arch" format
      echo "${1}"
      ;;
  esac
}

# Build Rust binary
build_rust() {
  local platform="$1"
  local os="${platform%%/*}"
  local arch="${platform##*/}"

  # Map to Rust target triple
  local target
  case "${os}-${arch}" in
    linux-amd64) target="x86_64-unknown-linux-gnu" ;;
    linux-arm64) target="aarch64-unknown-linux-gnu" ;;
    darwin-amd64) target="x86_64-apple-darwin" ;;
    darwin-arm64) target="aarch64-apple-darwin" ;;
    windows-amd64) target="x86_64-pc-windows-gnu" ;;
    windows-arm64)
      # Rust has no stable aarch64-pc-windows-gnu target (only gnullvm/msvc).
      echo "==> SKIP Rust windows/arm64: no stable GNU cross-target for aarch64 Windows"
      return 0
      ;;
    *)
      echo "Unsupported platform for Rust: ${os}/${arch}"
      return 1
      ;;
  esac

  # Cross-compiling for macOS from Linux requires the osxcross toolchain
  # (e.g. aarch64-apple-darwin21-clang). Without it, skip gracefully.
  if [[ "${os}" == "darwin" ]]; then
    local cc_name
    case "${arch}" in
      amd64) cc_name="x86_64-apple-darwin21-clang" ;;
      arm64) cc_name="aarch64-apple-darwin21-clang" ;;
    esac
    if ! command -v "${cc_name}" >/dev/null 2>&1; then
      echo "==> SKIP Rust ${os}/${arch}: ${cc_name} not found (install osxcross to enable)"
      return 0
    fi
  fi

  local out_name="firebird-5-mcp"
  local out_file="${BIN_DIR}/${out_name}"
  if [[ "${platform}" != "current" ]]; then
    out_file="${BIN_DIR}/${out_name}-${os}-${arch}"
  fi
  if [[ "${os}" == "windows" ]]; then
    out_file="${out_file}.exe"
  fi

  echo "==> Building Rust ${os}/${arch} (${target}) -> ${out_file}"
  (
    cd "${ROOT}"

    # For cross-compiling to aarch64-linux, we need to set the linker
    # explicitly via env var (the [target.*] linker in Cargo.toml is not
    # respected by rustc for cross-targets — it uses rust-lld which doesn't
    # support AArch64-specific flags on an x86_64 host).
    if [[ "${target}" == "aarch64-unknown-linux-gnu" ]]; then
      if command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
        export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
      else
        echo "==> SKIP Rust ${os}/${arch}: aarch64-linux-gnu-gcc not found"
        return 0
      fi
    fi

    if ! cargo build --release --target "${target}"; then
      echo "==> SKIP Rust ${os}/${arch}: cross-compile failed (target ${target})"
      return 0
    fi

    # The binary name in the target dir may or may not have .exe depending
    # on the target. Check both.
    local src="target/${target}/release/firebird-5-mcp"
    if [[ ! -f "${src}" && "${os}" == "windows" ]]; then
      src="target/${target}/release/firebird-5-mcp.exe"
    fi

    # Copy the binary. Skip if it's the same file (hardlink) or if the
    # destination is already up to date (Zed may have the binary open).
    if [[ "$(realpath "${src}")" == "$(realpath "${out_file}")" ]]; then
      : # same inode — nothing to do
    elif [[ -f "${out_file}" ]] && cmp -s "${src}" "${out_file}"; then
      : # already up to date
    else
      cp -f "${src}" "${out_file}"
    fi
  )
}

main() {
  local target="${1:-current}"

  local platforms
  platforms="$(resolve_platforms "${target}")"

  mkdir -p "${BIN_DIR}"

  for platform in ${platforms}; do
    build_rust "${platform}"
  done

  package_dist

  echo ""
  echo "Build complete."
  echo "Binaries in: ${BIN_DIR}"
  ls -lh "${BIN_DIR}"
  echo ""
  echo "Archives in: ${DIST_DIR}"
  ls -lh "${DIST_DIR}"
}

# Package built binaries into dist/ as compressed archives:
#   windows -> .zip, everything else -> .tar.gz
package_dist() {
  mkdir -p "${DIST_DIR}"

  local f base name
  for f in "${BIN_DIR}"/*; do
    [[ -f "${f}" ]] || continue
    base="$(basename "${f}")"

    # Strip the .exe suffix for the archive name
    name="${base%.exe}"

    if [[ "${base}" == *.exe ]]; then
      # Windows: zip the .exe
      (cd "${BIN_DIR}" && zip -q "${DIST_DIR}/${name}.zip" "${base}")
      echo "==> Packaged ${DIST_DIR}/${name}.zip"
    else
      # Linux/darwin: tar.gz the binary
      (cd "${BIN_DIR}" && tar czf "${DIST_DIR}/${name}.tar.gz" "${base}")
      echo "==> Packaged ${DIST_DIR}/${name}.tar.gz"
    fi
  done
}

main "$@"
