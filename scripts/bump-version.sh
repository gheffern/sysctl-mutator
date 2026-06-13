#!/usr/bin/env bash
set -euo pipefail

# Check if version argument is provided
if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <new-version>"
    echo "Example: $0 1.0.0"
    exit 1
fi

VERSION=$1

# Basic semantic version format validation (e.g. 1.0.0, 1.0.0-rc1)
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]]; then
    echo "Error: Version '$VERSION' is not a valid semantic version."
    exit 1
fi

echo "Bumping version to $VERSION..."

# 1. Update Cargo.toml version
if [ -f Cargo.toml ]; then
    sed -i 's/^version = "[^"]*"/version = "'"$VERSION"'"/' Cargo.toml
    echo "✓ Updated Cargo.toml"
else
    echo "Error: Cargo.toml not found"
    exit 1
fi

# 2. Update Helm Chart.yaml version and appVersion
CHART_PATH="k8s/charts/sysctl-mutator/Chart.yaml"
if [ -f "$CHART_PATH" ]; then
    sed -i 's/^version: .*/version: '"$VERSION"'/' "$CHART_PATH"
    sed -i 's/^appVersion: .*/appVersion: "'"$VERSION"'"/' "$CHART_PATH"
    echo "✓ Updated $CHART_PATH"
else
    echo "Error: $CHART_PATH not found"
    exit 1
fi

echo "Version bump complete!"
