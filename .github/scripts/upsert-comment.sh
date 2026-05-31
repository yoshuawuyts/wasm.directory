#!/usr/bin/env bash
# Post or update the registry-entry bot comment on an issue. Reuses a single
# marked comment so repeated edits/re-runs don't spam the thread.
#
# Required environment: REPO (owner/name), ISSUE (number), GH_TOKEN.
# Usage: upsert-comment.sh "<body markdown>"
set -euo pipefail

MARKER="<!-- registry-entry-bot -->"
BODY="${MARKER}
$1"

CID="$(gh api "repos/${REPO}/issues/${ISSUE}/comments" \
  --jq ".[] | select(.body | startswith(\"${MARKER}\")) | .id" | head -n1)"

if [ -n "$CID" ]; then
  gh api -X PATCH "repos/${REPO}/issues/comments/${CID}" -f body="$BODY" >/dev/null
else
  gh api -X POST "repos/${REPO}/issues/${ISSUE}/comments" -f body="$BODY" >/dev/null
fi
