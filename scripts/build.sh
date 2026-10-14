#!/usr/bin/env bash
#
# Build script for jed-personal-mcp (Go + Rust)
#
# Usage:
#   ./scripts/build.sh              # Build both Go and Rust for current platform
#   ./scripts/build.sh go           # Build only Go
#   ./scripts/build.sh rust         # Build only Rust
#   ./scripts/build.sh linux        # Build both for linux/amd64 + linux/arm64
#   ./scripts/build.sh darwin       # Build both for darwin/amd64 + darwin/arm64
#   ./scripts/build.sh windows      # Build both for windows/amd64 + windows/arm64
#   ./scripts/build.sh all          # Build both for all platforms
#
set -euo pipefail

# Resolve project root (parent of scripts/)
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${ROOT}/bin"
MODULE_NAME="$(basename "${ROOT}")"
GO_DIR="${ROOT}/golang"
RUST_DIR="${ROOT}/firebird-5-mcp"

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

# Build Go binary
build_go() {
  local platform="$1"
  local os="${platform%%/*}"
  local arch="${platform##*/}"

  local out_name="${MODULE_NAME}"
  if [[ "${os}" == "windows" ]]; then
    out_name="${out_name}.exe"
  fi

  local out_file="${BIN_DIR}/${out_name}"
  if [[ "${platform}" != "current" ]]; then
    out_file="${BIN_DIR}/${MODULE_NAME}-${os}-${arch}"
    if [[ "${os}" == "windows" ]]; then
      out_file="${out_file}.exe"
    fi
  fi

  echo "==> Building Go ${os}/${arch} -> ${out_file}"
  (
    cd "${GO_DIR}"
    GOOS="${os}" GOARCH="${arch}" CGO_ENABLED=0 \
      go build -trimpath -ldflags="-s -w" -o "${out_file}" .
  )
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
    windows-amd64) target="x86_64-pc-windows-msvc" ;;
    windows-arm64) target="aarch64-pc-windows-msvc" ;;
    *)
      echo "Unsupported platform for Rust: ${os}/${arch}"
      return 1
      ;;
  esac

  local out_name="firebird-5-mcp"
  if [[ "${os}" == "windows" ]]; then
    out_name="${out_name}.exe"
  fi

  local out_file="${BIN_DIR}/${out_name}"
  if [[ "${platform}" != "current" ]]; then
    out_file="${BIN_DIR}/${out_name}-${os}-${arch}"
    if [[ "${os}" == "windows" ]]; then
      out_file="${out_file}.exe"
    fi
  fi

  echo "==> Building Rust ${os}/${arch} (${target}) -> ${out_file}"
  (
    cd "${RUST_DIR}"
    cargo build --release --target "${target}" 2>/dev/null || \
    cargo build --release
    # Copy the binary
    local bin_path
    if [[ -d "target/${target}/release" ]]; then
      bin_path="target/${target}/release/firebird-5-mcp"
    else
      bin_path="target/release/firebird-5-mcp"
    fi
    cp "${bin_path}" "${out_file}"
  )
}

main() {
  local target="${1:-current}"
  local build_go_flag=true
  local build_rust_flag=true

  # Check if user specified which language to build
  case "${target}" in
    go)
      build_rust_flag=false
      target="current"
      ;;
    rust)
      build_go_flag=false
      target="current"
      ;;
  esac

  local platforms
  platforms="$(resolve_platforms "${target}")"

  mkdir -p "${BIN_DIR}"

  for platform in ${platforms}; do
    if [[ "${build_go_flag}" == true ]]; then
      build_go "${platform}"
    fi
    if [[ "${build_rust_flag}" == true ]]; then
      build_rust "${platform}"
    fi
  done

  echo ""
  echo "Build complete. Binaries in: ${BIN_DIR}"
  ls -lh "${BIN_DIR}"
}

main "$@"
