#!/usr/bin/env bash
# End-to-end check of the Android build on an emulator (run by .github/workflows/android.yml).
# A VLESS server is started on the host; the app (a debug build) is told through its autotest
# file to add that server, connect, stay connected for a while and disconnect. While it is
# connected another program on the phone makes a request, and the server's log has to show it:
# that proves the whole chain, system VPN -> tun2proxy -> the core on the phone -> the server.
#   tools/android-e2e.sh <apk>
set -u
APK="$1"
PKG=dev.quadra.client
OUT=android-e2e
UUID=5783a3e7-e373-51cd-8642-c83782b807c5
PORT=24443
HOLD=45
mkdir -p "$OUT"

# one plain HTTP request from the shell user, which is not our app and so goes through the VPN
request() {
  adb shell '{ printf "GET / HTTP/1.0\r\nHost: example.com\r\n\r\n"; sleep 6; } | nc -w 12 example.com 80 | head -1' | tr -d '\r'
}
read_app_file() {
  adb shell "run-as $PKG sh -c 'cat files/$1 $1 app_data/$1 2>/dev/null'" | tr -d '\r'
}

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
adb shell settings put global hide_error_dialogs 1   # a slow emulator's "isn't responding" boxes
adb install -r "$APK" || exit 1
# the consent dialog needs a finger; this is the same switch it flips
adb shell appops set $PKG ACTIVATE_VPN allow
adb shell pm grant $PKG android.permission.POST_NOTIFICATIONS 2>/dev/null || true

echo "== autotest file"
JOB="{\"link\":\"vless://$UUID@10.0.2.2:$PORT?type=tcp&security=none&encryption=none#Test\",\"connect\":true,\"hold\":$HOLD}"
for dir in files . app_data; do
  adb shell "run-as $PKG mkdir -p $dir" 2>/dev/null
  echo "$JOB" | adb shell "run-as $PKG sh -c 'cat > $dir/autotest.json'"
done

echo "== start"
adb logcat -c
adb shell am start -n $PKG/.MainActivity
sleep 10
adb exec-out screencap -p > "$OUT/1-started.png"

echo "== wait for the connection"
RESULT=""
for i in $(seq 1 30); do
  RESULT=$(read_app_file autotest-result.json)
  case "$RESULT" in *'"stage":"connect"'*) break ;; esac
  sleep 2
done
echo "autotest result: $RESULT"
echo "$RESULT" > "$OUT/result.json"
adb exec-out screencap -p > "$OUT/2-connected.png"

echo "== the system's view"
adb shell dumpsys connectivity 2>/dev/null | grep -oE "Transports: [A-Z|]*VPN[A-Z|]*|sessionId=[A-Za-z]*" | sort -u | head -4
TUN_UP=$(adb shell ip addr 2>/dev/null | grep -cE "^[0-9]+: tun[0-9]")

echo "== a request from another program on the phone, through the tunnel"
REPLY=$(request)
echo "reply: $REPLY"
adb exec-out screencap -p > "$OUT/3-traffic.png"

echo "== wait for the app to disconnect again"
GONE=""
for i in $(seq 1 40); do
  GONE=$(read_app_file autotest-disconnect.json)
  [ -n "$GONE" ] && break
  sleep 2
done
echo "after disconnect: $GONE"
sleep 3
TUN_AFTER=$(adb shell ip addr 2>/dev/null | grep -cE "^[0-9]+: tun[0-9]")
VPN_AFTER=$(adb shell dumpsys connectivity 2>/dev/null | grep -cE "sessionId=Quadra")
BEFORE_LINES=$(grep -c "example.com:80" "$OUT/server-access.log")
REPLY_DIRECT=$(request)
AFTER_LINES=$(grep -c "example.com:80" "$OUT/server-access.log")
echo "direct reply: $REPLY_DIRECT"
adb exec-out screencap -p > "$OUT/4-disconnected.png"

echo "== what the server saw"
tail -12 "$OUT/server-access.log" | cut -c1-140

echo "== app log"
read_app_file core.log | tail -25 | tee "$OUT/core.log"
adb logcat -d > "$OUT/logcat.txt" 2>/dev/null
grep -E "AndroidRuntime|FATAL EXCEPTION|panicked" "$OUT/logcat.txt" | tail -20

kill $SERVER 2>/dev/null

FAIL=0
check() { if [ "$1" = ok ]; then echo "PASS: $2"; else echo "FAIL: $2"; FAIL=1; fi; }
case "$RESULT" in *'"state":"on"'*) check ok "the app reports connected" ;; *) check no "the app reports connected" ;; esac
[ "$TUN_UP" -ge 1 ] && check ok "the tunnel device exists while connected" || check no "the tunnel device exists while connected"
[ "$BEFORE_LINES" -ge 1 ] && check ok "another program's request reached the server through the tunnel" || check no "another program's request reached the server through the tunnel"
case "$REPLY" in HTTP/*) check ok "and it got an answer ($REPLY)" ;; *) check no "and it got an answer" ;; esac
case "$GONE" in *'"state":"off"'*) check ok "the app reports disconnected" ;; *) check no "the app reports disconnected" ;; esac
[ "$TUN_AFTER" -eq 0 ] && [ "$VPN_AFTER" -eq 0 ] && check ok "the system VPN is gone after disconnecting" || check no "the system VPN is gone after disconnecting (tun=$TUN_AFTER vpn=$VPN_AFTER)"
case "$REPLY_DIRECT" in HTTP/*) check ok "the network still works afterwards ($REPLY_DIRECT)" ;; *) check no "the network still works afterwards" ;; esac
[ "$AFTER_LINES" -eq "$BEFORE_LINES" ] && check ok "and that request no longer goes through the server" || check no "and that request no longer goes through the server"
exit $FAIL
