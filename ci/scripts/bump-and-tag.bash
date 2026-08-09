#!/usr/bin/env bash

set -xeo pipefail

readonly live_run=${LIVE_RUN:-false}
# Release number
readonly version=${VERSION:?input VERSION is required}
# Git actor name
readonly git_user_name=${GIT_USER_NAME:?input GIT_USER_NAME is required}
# Git actor email
readonly git_user_email=${GIT_USER_EMAIL:?input GIT_USER_EMAIL is required}
# The zenoh-flat-jni release to build against, if it is moving with this release
readonly flat_jni_version=${FLAT_JNI_VERSION:-''}

export GIT_AUTHOR_NAME=$git_user_name
export GIT_AUTHOR_EMAIL=$git_user_email
export GIT_COMMITTER_NAME=$git_user_name
export GIT_COMMITTER_EMAIL=$git_user_email

# Bump Gradle project version. There is no Cargo manifest here any more: the
# native libraries live inside the zenoh-flat-jni artifact this SDK depends on.
printf '%s' "$version" > version.txt

git commit version.txt -m "chore: Bump version to \`$version\`"

# Point at the zenoh-flat-jni release this SDK is built against. It must be a
# real release, never a snapshot: consumers do not have the snapshot repository
# configured, and snapshots are mutable and eventually removed.
if [[ -n "$flat_jni_version" ]]; then
  case "$flat_jni_version" in
    *-SNAPSHOT)
      echo "error: refusing to release against a snapshot dependency ($flat_jni_version)" >&2
      exit 1
      ;;
  esac
  sed -i.bak -E "s|^zenohFlatJniVersion=.*|zenohFlatJniVersion=$flat_jni_version|" gradle.properties
  rm -f gradle.properties.bak
  git diff gradle.properties
  git commit gradle.properties -m "chore: Build against zenoh-flat-jni \`$flat_jni_version\`"
fi

if [[ ${live_run} ]]; then
  git tag --force "$version" -m "v$version"
fi
git log -10
git show-ref --tags
git push origin
git push --force origin "$version"
