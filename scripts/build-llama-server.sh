#!/usr/bin/env bash
# Compila llama-server (llama.cpp) come binario universale macOS (arm64 + x86_64)
# e lo posiziona in src-tauri/binaries/llama-server, da dove Tauri lo bundla
# come risorsa dell'app (vedi tauri.conf.json → bundle.resources, docs/architecture.md).
#
# Statico (BUILD_SHARED_LIBS=OFF) e con Metal (GGML_METAL=ON): nessuna dylib esterna
# da bundlare, accelerazione GPU su Apple Silicon e Intel. Web UI esclusa
# (LLAMA_BUILD_UI=OFF/LLAMA_USE_PREBUILT_UI=OFF): earwig non la usa, evita che la
# build scarichi asset o dipenda da npm.
#
# LLAMA_BUILD_LIBRESSL=ON: senza un backend TLS il binario compila ma fallisce
# a runtime con "HTTPS is not supported" su qualunque --hf-repo/--hf-file
# (llama.cpp non usa più libcurl, il downloader HF linka OpenSSL/BoringSSL/
# LibreSSL direttamente). LibreSSL vendorizzata dal build stesso (nessun
# `brew install openssl` da fare sulla macchina di sviluppo) è l'unica delle
# tre opzioni che non richiede una libreria di sistema già installata,
# coerente col resto di questo script (self-contained, nessuna dylib esterna).
#
# Uso: ./scripts/build-llama-server.sh [tag]
#   tag = tag/branch llama.cpp da compilare (default: b10229)
#
# Richiede: git, cmake, Xcode Command Line Tools (clang con supporto cross-arch macOS).

set -euo pipefail

LLAMA_TAG="${1:-b10229}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$ROOT_DIR/src-tauri/binaries"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

echo "==> Clono llama.cpp @ $LLAMA_TAG in $WORK_DIR"
git clone --depth 1 --branch "$LLAMA_TAG" https://github.com/ggml-org/llama.cpp.git "$WORK_DIR/llama.cpp"
cd "$WORK_DIR/llama.cpp"

CMAKE_FLAGS=(
  -DCMAKE_BUILD_TYPE=Release
  -DGGML_NATIVE=OFF
  -DGGML_METAL=ON
  -DBUILD_SHARED_LIBS=OFF
  -DLLAMA_BUILD_SERVER=ON
  -DLLAMA_BUILD_TESTS=OFF
  -DLLAMA_BUILD_EXAMPLES=OFF
  -DLLAMA_BUILD_UI=OFF
  -DLLAMA_USE_PREBUILT_UI=OFF
  -DLLAMA_BUILD_LIBRESSL=ON
)

for ARCH in arm64 x86_64; do
  echo "==> Compilo per $ARCH"
  cmake -B "build-$ARCH" -DCMAKE_OSX_ARCHITECTURES="$ARCH" "${CMAKE_FLAGS[@]}"
  cmake --build "build-$ARCH" --config Release --target llama-server -j"$(sysctl -n hw.ncpu)"
done

mkdir -p "$OUT_DIR"
echo "==> Unisco arm64 + x86_64 in binario universale → $OUT_DIR/llama-server"
lipo -create -output "$OUT_DIR/llama-server" \
  "build-arm64/bin/llama-server" \
  "build-x86_64/bin/llama-server"
chmod +x "$OUT_DIR/llama-server"

echo "==> Fatto:"
file "$OUT_DIR/llama-server"
lipo -info "$OUT_DIR/llama-server"
