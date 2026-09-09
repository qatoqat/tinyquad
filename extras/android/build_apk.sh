#!/usr/bin/env bash
# Build an installable APK for the `quad_android` example.
#
# This reimplements the packaging half of cargo-quad-apk (java substitution,
# dexing, aapt packaging, zipalign, debug signing) with the plain Android SDK
# build-tools. cargo-quad-apk itself cannot be used here: it statically links
# a cargo library too old to parse edition-2024 manifests, and there is no
# newer release.
#
# Environment:
#   ANDROID_HOME         SDK root. Defaults to /usr/local/lib/android/sdk
#                        (GitHub Actions runners), falling back to
#                        $HOME/android-sdk.
#   NDK_VERSION          NDK directory name under $ANDROID_HOME/ndk.
#                        Defaults to the newest directory found.
#   BUILD_TOOLS_VERSION  defaults to 34.0.0
#   PLATFORM             defaults to android-34
#   TARGET               rust target, defaults to aarch64-linux-android
#   ANDROID_ABI          jni abi dir, defaults to arm64-v8a
#   APK_OUT              where to put the final apk, defaults to
#                       <repo>/target/android/quad_android.apk
#
# Requires java (javac/keytool) on PATH; use e.g. actions/setup-java in CI.
set -euo pipefail

: "${ANDROID_HOME:=${HOME}/android-sdk}"
if [ -z "${NDK_VERSION:-}" ]; then
    NDK_VERSION="$(ls "${ANDROID_HOME}/ndk" 2>/dev/null | sort -V | tail -1)"
fi
: "${BUILD_TOOLS_VERSION:=34.0.0}"
: "${PLATFORM:=android-34}"
: "${TARGET:=aarch64-linux-android}"
: "${ANDROID_ABI:=arm64-v8a}"

BUILD_TOOLS="${ANDROID_HOME}/build-tools/${BUILD_TOOLS_VERSION}"
SYSROOT_JAR="${ANDROID_HOME}/platforms/${PLATFORM}/android.jar"
NDK="${ANDROID_HOME}/ndk/${NDK_VERSION}"
NDK_BIN="${NDK}/toolchains/llvm/prebuilt/linux-x86_64/bin"

for tool in "${BUILD_TOOLS}/aapt" "${BUILD_TOOLS}/d8" "${BUILD_TOOLS}/zipalign" \
            "${BUILD_TOOLS}/apksigner" "${SYSROOT_JAR}" "${NDK_BIN}"; do
    if [ ! -e "${tool}" ]; then
        echo "missing ${tool} - check ANDROID_HOME/NDK_VERSION/BUILD_TOOLS_VERSION/PLATFORM" >&2
        exit 1
    fi
done

# The rust->android linker driver name differs between architectures.
case "${TARGET}" in
    aarch64-linux-android)    LINKER_PREFIX=aarch64-linux-android ;;
    armv7-linux-androideabi)  LINKER_PREFIX=armv7a-linux-androideabi ;;
    *)                        LINKER_PREFIX="${TARGET}" ;;
esac
API=24
LINKER="${LINKER_PREFIX}${API}-clang"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
STAGING="${REPO_ROOT}/target/android/staging"
: "${APK_OUT:=${REPO_ROOT}/target/android/quad_android.apk}"
PACKAGE="org.tinyquad.example"
LIBRARY_NAME="quad_android"

rm -rf "${STAGING}"
mkdir -p "${STAGING}/src/${PACKAGE//./\/}" "${STAGING}/src/quad_native" \
         "${STAGING}/classes" "${STAGING}/dex" "${STAGING}/lib/${ANDROID_ABI}"

echo "== building ${LIBRARY_NAME}.so for ${TARGET} =="
# The NDK's clang wrappers resolve their versioned clang via PATH.
( cd "${REPO_ROOT}" && PATH="${NDK_BIN}:${PATH}" RUSTFLAGS="-C linker=${NDK_BIN}/${LINKER}" \
    cargo build --release --target "${TARGET}" --example quad_android )
cp "${REPO_ROOT}/target/${TARGET}/release/examples/lib${LIBRARY_NAME}.so" \
   "${STAGING}/lib/${ANDROID_ABI}/"

echo "== compiling java sources =="
sed -e "s/TARGET_PACKAGE_NAME/${PACKAGE}/" -e "s/LIBRARY_NAME/${LIBRARY_NAME}/" \
    "${REPO_ROOT}/java/MainActivity.java" > "${STAGING}/src/${PACKAGE//./\/}/MainActivity.java"
cp "${REPO_ROOT}/java/QuadNative.java" "${STAGING}/src/quad_native/"

javac --release 11 -classpath "${SYSROOT_JAR}" -d "${STAGING}/classes" \
      $(find "${STAGING}/src" -name "*.java")

"${BUILD_TOOLS}/d8" --release --min-api "${API}" --output "${STAGING}/dex" \
    $(find "${STAGING}/classes" -name "*.class")

echo "== packaging apk =="
cp "${REPO_ROOT}/extras/android/AndroidManifest.xml" "${STAGING}/"
cp "${STAGING}/dex/classes.dex" "${STAGING}/classes.dex"
( cd "${STAGING}" \
    && "${BUILD_TOOLS}/aapt" package -f -M AndroidManifest.xml -I "${SYSROOT_JAR}" -F unsigned.apk \
    && "${BUILD_TOOLS}/aapt" add unsigned.apk classes.dex "lib/${ANDROID_ABI}/lib${LIBRARY_NAME}.so" )

echo "== signing =="
KEYSTORE="${STAGING}/debug.keystore"
if [ ! -e "${KEYSTORE}" ]; then
    keytool -genkeypair -keystore "${KEYSTORE}" -storepass android -keypass android \
        -alias androiddebugkey -keyalg RSA -validity 10000 \
        -dname "CN=Android Debug,O=Android,C=US"
fi

"${BUILD_TOOLS}/zipalign" -f 4 "${STAGING}/unsigned.apk" "${STAGING}/aligned.apk"
"${BUILD_TOOLS}/apksigner" sign \
    --ks "${KEYSTORE}" --ks-pass pass:android --key-pass pass:android \
    --min-sdk-version "${API}" \
    --out "${APK_OUT}" "${STAGING}/aligned.apk"
"${BUILD_TOOLS}/apksigner" verify --print-certs "${APK_OUT}" | head -3

echo "== apk contents =="
"${BUILD_TOOLS}/aapt" list "${APK_OUT}"
echo "APK written to ${APK_OUT}"
