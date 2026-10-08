#!/usr/bin/env bash
# End-to-end check of the Android build on an emulator (run by .github/workflows/android.yml).
# A VLESS server is started on the host; the app (a debug build) is told through its autotest
# file to add that server and connect; then another program on the phone makes a request, and
# the server's log has to show it. That proves the whole chain: system VPN -> tun2proxy -> the
# core on the phone -> the server.
#   tools/android-e2e.sh <apk>
set -u
APK="$1"
PKG=dev.quadra.client
OUT=android-e2e
UUID=5783a3e7-e373-51cd-8642-c83782b807c5
PORT=24443
mkdir -p "$OUT"

echo "== test server on the host"
node tools/fetch-xray.mjs >/dev/null
cat > "$OUT/server.json" <<EOF
{
  "log": {"loglevel": "info", "access": "$PWD/$OUT/server-access.log"},
  "inbounds": [{"listen": "0.0.0.0", "port": $PORT, "protocol": "vless",
    "settings": {"clients": [{"id": "$UUID"}], "decryption": "none"},
    "streamSettings": {"network": "tcp", "security": "none"}}],
  "outbounds": [{"protocol": "freedom"}]
}
EOF
: > "$OUT/server-access.log"
src-tauri/resources/xray/xray run -c "$OUT/server.json" > "$OUT/server.log" 2>&1 &
SERVER=$!
sleep 2
kill -0 $SERVER || { echo "server did not start"; cat "$OUT/server.log"; exit 1; }

echo "== install"
adb install -r "$APK" || exit 1
# the consent dialog needs a finger; this is the same switch it flips
adb shell appops set $PKG ACTIVATE_VPN allow
adb shell pm grant $PKG android.permission.POST_NOTIFICATIONS 2>/dev/null || true

echo "== autotest file"
JOB="{\"link\":\"vless://$UUID@10.0.2.2:$PORT?type=tcp&security=none&encryption=none#Test\",\"connect\":true}"
for dir in files . app_data; do
  adb shell "run-as $PKG mkdir -p $dir" 2>/dev/null
  echo "$JOB" | adb shell "run-as $PKG sh -c 'cat > $dir/autotest.json'"
done

echo "== start"
adb logcat -c
adb shell am start -n $PKG/.MainActivity
sleep 12
adb exec-out screencap -p > "$OUT/1-started.png"

echo "== wait for the connection"
RESULT=""
for i in $(seq 1 30); do
  RESULT=$(adb shell "run-as $PKG sh -c 'cat files/autotest-result.json autotest-result.json app_data/autotest-result.json 2>/dev/null'" | tr -d '\r')
  case "$RESULT" in *'"stage":"connect"'*) break ;; esac
  sleep 3
done
echo "autotest result: $RESULT"
echo "$RESULT" > "$OUT/result.json"
adb exec-out screencap -p > "$OUT/2-connected.png"

echo "== the system's view"
adb shell dumpsys connectivity 2>/dev/null | grep -iE "VPN\b.*CONNECTED|type: VPN|Transports: VPN" | head -5
adb shell ip addr 2>/dev/null | grep -A3 -E "tun[0-9]" | head -8

echo "== a request from another program on the phone"
REPLY=$(adb shell 'printf "GET / HTTP/1.0\r\nHost: example.com\r\n\r\n" | nc -w 10 example.com 80 | head -1' | tr -d '\r')
echo "reply: $REPLY"
sleep 2
adb exec-out screencap -p > "$OUT/3-traffic.png"

echo "== what the server saw"
cat "$OUT/server-access.log" | tail -20

echo "== app log"
adb shell "run-as $PKG sh -c 'cat files/core.log core.log app_data/core.log 2>/dev/null'" | tail -30 | tee "$OUT/core.log"
adb logcat -d > "$OUT/logcat.txt" 2>/dev/null
grep -E "RustStdoutStderr|AndroidRuntime|FATAL|quadra|Quadra|Tauri" "$OUT/logcat.txt" | tail -60

kill $SERVER 2>/dev/null

FAIL=0
case "$RESULT" in *'"state":"on"'*) echo "PASS: the app reports connected" ;; *) echo "FAIL: the app did not connect"; FAIL=1 ;; esac
if grep -q "example.com:80" "$OUT/server-access.log"; then echo "PASS: another program's traffic reached the server through the tunnel"; else echo "FAIL: no tunnelled traffic in the server log"; FAIL=1; fi
case "$REPLY" in HTTP/*) echo "PASS: and it got an answer ($REPLY)" ;; *) echo "FAIL: no answer to the request"; FAIL=1 ;; esac
exit $FAIL
