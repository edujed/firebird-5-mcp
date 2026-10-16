#!/usr/bin/env bash
#
# Publish a new release tag.
#
# Usage:
#   ./scripts/publish_new_tag.sh <tag> <commit-message>
#
# Example:
#   ./scripts/publish_new_tag.sh v1.2.0 "feat: add new feature"
#
# What it does:
#   1. Stages all changes (git add -A)
#   2. Commits with the given message
#   3. Creates an annotated tag on the commit
#   4. Pushes the branch and the tag to origin
#
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <tag> <commit-message>"
  echo ""
  echo "Example:"
  echo "  $0 v1.2.0 \"feat: add new feature\""
  exit 1
fi

TAG="$1"
MSG="$2"

# Resolve project root (parent of scripts/)
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

# Check we're on a branch (not detached HEAD)
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [[ "${BRANCH}" == "HEAD" ]]; then
  echo "Error: not on a branch (detached HEAD)."
  exit 1
fi

# Check tag doesn't already exist
if git rev-parse "refs/tags/${TAG}" >/dev/null 2>&1; then
  echo "Error: tag '${TAG}' already exists."
  echo "Delete it first: git tag -d ${TAG} && git push origin :refs/tags/${TAG}"
  exit 1
fi

# Check there's something to commit
if git diff --quiet && git diff --cached --quiet; then
  echo "Nothing to commit. Working tree is clean."
  echo "If you just want to tag the current commit, use:"
  echo "  git tag -a ${TAG} -m '${MSG}' && git push origin ${TAG}"
  exit 0
fi

echo "==> Staging changes..."
git add -A

echo "==> Committing: ${MSG}"
git commit -m "${MSG}"

echo "==> Creating tag: ${TAG}"
git tag -a "${TAG}" -m "${MSG}"

echo "==> Pushing branch '${BRANCH}' and tag '${TAG}' to origin..."
git push origin "${BRANCH}"
git push origin "${TAG}"

echo ""
echo "Done. Release ${TAG} published on branch ${BRANCH}."
echo "GitHub Actions should start building now."
