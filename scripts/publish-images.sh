#!/usr/bin/env bash
# Build and push backend + frontend container images to GitHub Container Registry.
#
# Prerequisites:
#   - Docker (with buildx)
#   - Authenticated to ghcr.io:  echo $GITHUB_TOKEN | docker login ghcr.io -u <user> --password-stdin
#
# Usage:
#   ./scripts/publish-images.sh                  # uses defaults (repo=duffney/component-cli, tag=latest)
#   ./scripts/publish-images.sh -t v1.0.0        # custom tag
#   ./scripts/publish-images.sh -r myorg/myrepo  # custom repo
set -euo pipefail

REPO="duffney/component-cli"
TAG="latest"

while getopts "r:t:" opt; do
  case $opt in
    r) REPO="$OPTARG" ;;
    t) TAG="$OPTARG" ;;
    *) echo "Usage: $0 [-r repo] [-t tag]" >&2; exit 1 ;;
  esac
done

REGISTRY="ghcr.io/${REPO}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "==> Building and pushing images to ${REGISTRY}"
echo "    Tag: ${TAG}"
echo ""

# Backend
BACKEND_IMAGE="${REGISTRY}/backend:${TAG}"
echo "==> Building backend: ${BACKEND_IMAGE}"
docker build \
  -f "$ROOT_DIR/Dockerfile.backend" \
  -t "$BACKEND_IMAGE" \
  --platform linux/amd64 \
  "$ROOT_DIR"

echo "==> Pushing backend"
docker push "$BACKEND_IMAGE"
echo ""

# Frontend — API_BASE_URL is baked into the Wasm binary at build time.
# Default to http://backend which resolves inside Azure Container Apps.
FRONTEND_IMAGE="${REGISTRY}/frontend:${TAG}"
API_BASE_URL="${API_BASE_URL:-http://backend}"
echo "==> Building frontend: ${FRONTEND_IMAGE}"
echo "    API_BASE_URL=${API_BASE_URL}"
docker build \
  -f "$ROOT_DIR/Dockerfile.frontend" \
  -t "$FRONTEND_IMAGE" \
  --platform linux/amd64 \
  --build-arg "API_BASE_URL=${API_BASE_URL}" \
  "$ROOT_DIR"

echo "==> Pushing frontend"
docker push "$FRONTEND_IMAGE"
echo ""

echo "==> Done!"
echo "    Backend:  ${BACKEND_IMAGE}"
echo "    Frontend: ${FRONTEND_IMAGE}"
