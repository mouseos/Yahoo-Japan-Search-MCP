#!/usr/bin/env bash
# Pack MCPB bundles for each platform (ported from Baidu_MCP's
# scripts/pack-mcpb.sh so all search MCPs share the same packing shape;
# bundle file naming follows this repo's server.json / release.yml:
# yahoo-search-mcp-v<version>-<win32|macos|linux>-<arch>.mcpb).
# Usage: ./scripts/pack-mcpb.sh [version]
# Uses cross-compiled binaries in target/<triple>/release/ when present,
# falling back to the local target/release build for the host platform.
set -euo pipefail

VERSION="${1:-0.1.0}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"
rm -rf "$DIST"
mkdir -p "$DIST"

# triple bundle_platform arch exe manifest_platform
TARGETS=(
  "x86_64-pc-windows-msvc win32 x64 yahoo-search-mcp.exe win32"
  "aarch64-apple-darwin macos arm64 yahoo-search-mcp darwin"
  "x86_64-unknown-linux-gnu linux x64 yahoo-search-mcp linux"
)

RUSTC="$(command -v rustc || echo "$HOME/.cargo/bin/rustc")"
HOST_TRIPLE="$("$RUSTC" -vV 2>/dev/null | sed -n 's/^host: //p' || true)"
if [ -z "$HOST_TRIPLE" ]; then
  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) HOST_TRIPLE="x86_64-unknown-linux-gnu" ;;
    Linux-aarch64) HOST_TRIPLE="aarch64-unknown-linux-gnu" ;;
    Darwin-arm64) HOST_TRIPLE="aarch64-apple-darwin" ;;
    Darwin-x86_64) HOST_TRIPLE="x86_64-apple-darwin" ;;
    MINGW*-x86_64|MSYS*-x86_64) HOST_TRIPLE="x86_64-pc-windows-msvc" ;;
    *) HOST_TRIPLE="unknown" ;;
  esac
fi

for row in "${TARGETS[@]}"; do
  read -r triple platform arch exe manifest_platform <<<"$row"
  bin="$ROOT/target/$triple/release/$exe"
  if [ ! -f "$bin" ]; then
    # Fall back to the plain target/release build ONLY when it is a build
    # for this very target (i.e. the host itself). Never relabel a binary
    # built for another platform.
    local_bin="$ROOT/target/release/$exe"
    if [ "$triple" = "$HOST_TRIPLE" ] && [ -f "$local_bin" ]; then
      bin="$local_bin"
      echo "[INFO] Using local release build for $platform-$arch"
    else
      echo "[SKIP] Binary not found for $triple: $bin"
      continue
    fi
  fi

  name="yahoo-search-mcp-v$VERSION-$platform-$arch"
  stage="$DIST/$name-stage"
  mkdir -p "$stage/server"
  cp "$bin" "$stage/server/$exe"

  # manifest.json with platform-specific entry_point / command / platforms
  python3 - "$ROOT/manifest.json" "$stage/manifest.json" "$VERSION" "$exe" "$manifest_platform" <<'PY'
import json, sys
src, dst, version, exe, platform = sys.argv[1:6]
m = json.load(open(src))
m["version"] = version
m["server"]["entry_point"] = f"server/{exe}"
m["server"]["mcp_config"]["command"] = f"server/{exe}"
m["compatibility"]["platforms"] = [platform]
json.dump(m, open(dst, "w"), ensure_ascii=False, indent=2)
PY

  (cd "$stage" && zip -qr "$DIST/$name.mcpb" .)
  rm -rf "$stage"
  hash="$(sha256sum "$DIST/$name.mcpb" | cut -d' ' -f1)"
  size="$(stat -c%s "$DIST/$name.mcpb")"
  echo "[OK] $name.mcpb ($size bytes, sha256: $hash)"
done

echo
echo "Done. Bundles are in: $DIST"
echo "Update server.json fileSha256 fields with the hashes above before publishing."
