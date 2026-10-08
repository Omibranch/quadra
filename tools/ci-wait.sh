#!/usr/bin/env bash
# Waits for a GitHub Actions run and prints, for each job, the result and the end of the log of
# the step that failed.   tools/ci-wait.sh <run id> [lines of log]
run="$1"; lines="${2:-70}"
for i in $(seq 1 110); do
  status=$(timeout 40 gh run view "$run" --json status --jq .status 2>/dev/null)
  [ "$status" = "completed" ] && break
  sleep 8
done
timeout 60 gh run view "$run" --json jobs --jq '.jobs[] | "\(.name): \(.status) \(.conclusion) | failed step: \([.steps[] | select(.conclusion=="failure") | .name] | join(", "))"'
if [ "$(timeout 40 gh run view "$run" --json conclusion --jq .conclusion 2>/dev/null)" != "success" ]; then
  echo "----- end of the failed step's log"
  timeout 120 gh run view "$run" --log-failed 2>/dev/null | cut -f3- | sed 's/^[0-9T:.Z-]* //' | tail -"$lines"
fi
