#!/usr/bin/env bash
# End-to-end check against a live Cosmonic Desktop daemon.
#
#   COSMONIC_STATE_DIR=... scripts/e2e.sh
#
# Needs the daemon running with the mock backend, so the script can answer on
# the user's behalf:
#
#   COSMONIC_NOTIFY_BACKEND=mock COSMONIC_FLAG_SETTINGS_NOTIFICATIONS=1 cosmonicd run
set -euo pipefail

SOCK="${COSMONIC_STATE_DIR:?set COSMONIC_STATE_DIR}/cosmonicd.sock"
INGRESS="${COSMONIC_INGRESS:-127.0.0.1:8200}"
HOST="${NOTIFY_HOST:-notify.localhost}"

api()  { curl -sS --unix-socket "$SOCK" "$@"; }
page() { curl -sS -H "Host: $HOST" "$@"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

echo "1. the component can read the host's capabilities"
CAPS=$(page "http://$INGRESS/capabilities")
echo "   $CAPS"
echo "$CAPS" | grep -q '"backend":"mock"' || fail "expected the mock backend; got $CAPS"

echo "2. host-side validation rejects a non-http URL target"
OUT=$(page -X POST "http://$INGRESS/post" \
  -d '{"title":"x","actions":[{"id":"a","label":"a","target":"url","value":"javascript:alert(1)"}]}')
echo "$OUT" | grep -q '"error"' || fail "a javascript: URL must be rejected; got $OUT"

echo "3. host-side validation rejects a traversing deep link"
OUT=$(page -X POST "http://$INGRESS/post" \
  -d '{"title":"x","actions":[{"id":"a","label":"a","target":"deep-link","value":"../../etc/passwd"}]}')
echo "$OUT" | grep -q '"error"' || fail "a traversing route must be rejected; got $OUT"

echo "4. post → simulate → events.pull round trip"
ID=$(page -X POST "http://$INGRESS/post" \
  -d '{"title":"e2e","actions":[{"id":"approve","label":"Approve","target":"callback"}]}' \
  | sed 's/.*"id"://; s/}.*//')
[ -n "$ID" ] || fail "post did not return an id"
api -X POST -H 'content-type: application/json' \
  -d "{\"id\":$ID,\"kind\":\"action\",\"actionId\":\"approve\"}" \
  http://localhost/v1/notify/simulate > /dev/null
sleep 1
EVENTS=$(page "http://$INGRESS/events")
echo "$EVENTS" | grep -q '"kind":"action"' || fail "the response never came back: $EVENTS"
echo "   $EVENTS"

echo "5. an unanswered post still terminates"
ID=$(page -X POST "http://$INGRESS/post" -d '{"title":"ignored","timeout_ms":1500}' \
  | sed 's/.*"id"://; s/}.*//')
sleep 3
page "http://$INGRESS/events" | grep -q '"kind":"expired"' \
  || fail "an ignored notification must expire on its own"

echo "6. revoking the workload denies it, and survives"
api -X PUT -H 'content-type: application/json' -d '{"allowed":false}' \
  "http://localhost/v1/notify/components/default%2Fnotify-playground" > /dev/null
page -X POST "http://$INGRESS/post" -d '{"title":"denied?"}' | grep -q 'denied' \
  || fail "a revoked workload must be denied"
api -X PUT -H 'content-type: application/json' -d '{"allowed":true}' \
  "http://localhost/v1/notify/components/default%2Fnotify-playground" > /dev/null

echo
echo "all checks passed"
