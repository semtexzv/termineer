#!/bin/bash
set -e

# Check for a clean working directory
if ! git diff-index --quiet HEAD --; then
  echo "Working directory is not clean. Please commit or stash your changes."
  exit 1
fi

# Default behavior: increment patch version if no version specified
NEW_VERSION=""

# Function to display usage information
function show_help {
  echo "Termineer Release Script"
  echo "Usage: $0 [options]"
  echo
  echo "Options:"
  echo "  -v, --version VERSION   Specify version to release (e.g., 0.1.15)"
  echo "  -h, --help              Show this help message"
  echo
  echo "If no version is specified, the script will increment the patch version."
}

# Process command line arguments
while [[ $# -gt 0 ]]; do
  case "$1" in
    -v|--version)
      NEW_VERSION="$2"
      shift 2
      ;;
    -h|--help)
      show_help
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      show_help
      exit 1
      ;;
  esac
done

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep -E '^version = "[0-9]+\.[0-9]+\.[0-9]+"' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
echo "Current version: $CURRENT_VERSION"

# If no version specified, increment patch version
if [[ -z "$NEW_VERSION" ]]; then
  # Split version into parts
  IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
  
  # Increment patch version
  PATCH=$((PATCH + 1))
  
  # Construct new version
  NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}"
  echo "No version specified. Incrementing patch version to: $NEW_VERSION"
fi

# Ensure the new version is valid
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Error: Invalid version format. Please use semantic versioning (e.g., 0.1.15)"
  exit 1
fi

# Update version in Cargo.toml
echo "Updating version in Cargo.toml to $NEW_VERSION"
sed -i.bak "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
rm Cargo.toml.bak

# Update version in README.md if it exists
if grep -q "$CURRENT_VERSION" README.md 2>/dev/null; then
  echo "Updating version in README.md"
  sed -i.bak "s/$CURRENT_VERSION/$NEW_VERSION/g" README.md
  rm README.md.bak
fi

# Update version in .termineer/info if it exists
if grep -q "$CURRENT_VERSION" .termineer/info 2>/dev/null; then
  echo "Updating version in .termineer/info"
  sed -i.bak "s/$CURRENT_VERSION/$NEW_VERSION/g" .termineer/info
  rm .termineer/info.bak
fi

# Commit changes
echo "Committing changes"
git add Cargo.toml README.md .termineer/info 2>/dev/null || true
git commit -m "Bump version to $NEW_VERSION for release"

# Create tag with -npm suffix to trigger NPM release
TAG_NAME="v$NEW_VERSION-npm"
echo "Creating tag: $TAG_NAME"
git tag "$TAG_NAME"

# Push commit and tag
echo "Pushing commit and tag to origin"
git push origin master
git push origin "$TAG_NAME"

echo "Release process initiated for version $NEW_VERSION"
echo "GitHub Actions workflow should now be running to build and publish the release"
echo "You can monitor progress at: https://github.com/$(git config --get remote.origin.url | sed 's/.*github.com[:\/]\(.*\)\.git/\1/')/actions"