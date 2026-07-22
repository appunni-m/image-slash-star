#!/usr/bin/env bash
# Build the exact native AVIF stack used by Pillow 12.2.0.

set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "usage: $0 INSTALL_PREFIX WORK_DIRECTORY" >&2
    exit 2
fi

install_prefix=$1
work_directory=$2
source_directory="$work_directory/libavif-1.4.1"
build_directory="$work_directory/build"
expected_libavif_commit=6543b22b5bc706c53f038a16fe515f921556d9b3

for command in cmake git meson ninja pkg-config; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "required build command is unavailable: $command" >&2
        exit 1
    fi
done

mkdir -p "$install_prefix" "$work_directory"

if [[ ! -d "$source_directory/.git" ]]; then
    git clone --branch v1.4.1 --depth 1 \
        https://github.com/AOMediaCodec/libavif.git "$source_directory"
fi

actual_libavif_commit=$(git -C "$source_directory" rev-parse HEAD)
if [[ "$actual_libavif_commit" != "$expected_libavif_commit" ]]; then
    echo "unexpected libavif commit: $actual_libavif_commit" >&2
    exit 1
fi

cmake -S "$source_directory" -B "$build_directory" \
    -DCMAKE_BUILD_TYPE=MinSizeRel \
    -DCMAKE_INSTALL_PREFIX="$install_prefix" \
    -DCMAKE_INSTALL_LIBDIR=lib \
    -DCMAKE_INTERPROCEDURAL_OPTIMIZATION=ON \
    -DCMAKE_C_VISIBILITY_PRESET=hidden \
    -DCMAKE_CXX_VISIBILITY_PRESET=hidden \
    -DBUILD_SHARED_LIBS=ON \
    -DAVIF_BUILD_APPS=OFF \
    -DAVIF_BUILD_TESTS=OFF \
    -DAVIF_CODEC_AOM=LOCAL \
    -DAVIF_CODEC_AOM_DECODE=OFF \
    -DAVIF_CODEC_AOM_ENCODE=ON \
    -DAVIF_CODEC_DAV1D=LOCAL \
    -DAVIF_LIBSHARPYUV=LOCAL \
    -DAVIF_LIBYUV=LOCAL \
    -DCONFIG_AV1_HIGHBITDEPTH=0

cmake --build "$build_directory" --parallel 4
cmake --install "$build_directory"

installed_version=$(
    PKG_CONFIG_PATH="$install_prefix/lib/pkgconfig" \
        pkg-config --modversion libavif
)
if [[ "$installed_version" != "1.4.1" ]]; then
    echo "unexpected installed libavif version: $installed_version" >&2
    exit 1
fi
