#!/usr/bin/env bash
# Starts a release APK on the emulator and checks that it stays up and draws its screen.
# The release build has no autotest hook, so this is all that can be asked of it unattended.
#   tools/android-smoke.sh <apk>
set -u
APK="$1"
PKG=dev.quadra.client
OUT=android-e2e
mkdir -p "$OUT"

adb uninstall $PKG >/dev/null 2>&1    # the debug build is signed with another key
adb install "$APK" || exit 1
adb logcat -c
adb shell am start -n $PKG/.MainActivity
sleep 20
adb exec-out screencap -p > "$OUT/5-release-build.png"
adb logcat -d > "$OUT/logcat-release.txt" 2>/dev/null
grep -E "AndroidRuntime|FATAL EXCEPTION|panicked" "$OUT/logcat-release.txt" | tail -20
if adb shell pidof $PKG >/dev/null; then
  echo "PASS: the release build is running after 20 s"
else
  echo "FAIL: the release build is not running"
  exit 1
fi
