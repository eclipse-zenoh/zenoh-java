#!/usr/bin/env bash
#
# Is the zenoh-flat-jni copy this repository publishes already the commit we pin?
#
# The SDK snapshot names org.eclipse.zenoh:zenoh-flat-jni:<qualified version>,
# and this repository is what puts that coordinate there — built from the commit
# Cargo.lock pins, so the dependency exists and is the code the SDK compiled
# against, whether or not zenoh-flat-jni's CI has ever run. Producing it means
# cross-compiling ten targets, on the order of half an hour, so it is rebuilt
# only when the published copy is not already that commit.
#
# The published copy says which commit it was built from as a POM property, so
# the check costs two small requests per coordinate rather than a 39 MB
# download. Anything missing — no metadata, no POM, no stamp — reads as "not
# ours" and rebuilds, which is the safe direction.
#
# Writes `commit`, `version`, `base`, `qualifier` and `rebuild` to $GITHUB_OUTPUT
# when running under Actions, and prints them either way. `--self-test` runs the
# parsers against fixtures and exits.
#
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

readonly repository=${SNAPSHOT_REPOSITORY:-https://central.sonatype.com/repository/maven-snapshots}
readonly group_path=org/eclipse/zenoh
# The qualifier that keeps our copy from overwriting zenoh-flat-jni's own
# snapshot or zenoh-kotlin's. Checked against gradle.properties below, and
# handed to zenoh-flat-jni's publication workflow, so the two cannot drift.
readonly qualifier=${FLAT_JNI_QUALIFIER:-java}
# All three coordinates, not just the root: one Gradle invocation uploads them,
# but a partial failure leaves them at different builds, and it is the platform
# ones that carry the native libraries.
#
# Each with the extension of its binary, because a matching stamp is not on its
# own proof of a finished publication. The POM goes up before the Gradle module
# metadata and before the jar or aar, so a run that died in between would leave
# three readable stamps, no module metadata for Gradle to resolve a variant
# against, and a decision to skip rebuilding — forever, since the next run reads
# the same three stamps. Nothing downstream would catch it either: the consumer
# smoke test runs on Linux, and the broken variant could be the Android one.
readonly artifacts=(
    zenoh-flat-jni:jar
    zenoh-flat-jni-jvm:jar
    zenoh-flat-jni-android:aar
)

# The commit Cargo.lock pins for the git dependency on zenoh-flat-jni. Reads
# stdin so it can be tested without a lockfile.
pinned_commit() {
    grep -Eom1 'zenoh-flat-jni\.git[^#"]*#[0-9a-f]{40}' | grep -Eo '[0-9a-f]{40}$'
}

# The file name the newest timestamped build of a snapshot has, from that
# version's maven-metadata.xml on stdin:
#   <artifact>-<version without -SNAPSHOT>-<timestamp>-<buildNumber>.<ext>
timestamped_name() { # <artifact> <version> <ext>
    local metadata timestamp build
    metadata=$(cat)
    timestamp=$(sed -n 's:.*<timestamp>\(.*\)</timestamp>.*:\1:p' <<<"$metadata" | head -1)
    build=$(sed -n 's:.*<buildNumber>\(.*\)</buildNumber>.*:\1:p' <<<"$metadata" | head -1)
    [[ -n $timestamp && -n $build ]] || return 1
    printf '%s-%s-%s-%s.%s' "$1" "${2%-SNAPSHOT}" "$timestamp" "$build" "$3"
}

# The commit a published POM on stdin was built from; empty when it has no stamp.
pom_commit() {
    sed -n 's:.*<zenoh\.flatJniCommit>\(.*\)</zenoh\.flatJniCommit>.*:\1:p' | head -1
}

# Whether the metadata on stdin advertises an unclassified artifact of each given
# extension. Maven appends an entry as each file lands, so a publication that
# stopped part-way lists fewer than a finished one. The anchor is what excludes
# the sources and javadoc jars: the schema puts <classifier> before <extension>,
# so only a main artifact starts its entry with the extension.
advertises() { # <extension>…
    local blocks ext
    blocks=$(tr -d '[:space:]' | sed 's:<snapshotVersion>:\n:g')
    for ext in "$@"; do
        grep -q "^<extension>$ext</extension>" <<<"$blocks" || return 1
    done
}

# The stamp of the published copy of one coordinate; empty if it is not there, or
# not all of it is.
published_commit() { # <artifact> <version> <binary extension>
    local base_url="$repository/$group_path/$1/$2" metadata name
    metadata=$(curl -sf "$base_url/maven-metadata.xml") || return 0
    advertises pom module "$3" <<<"$metadata" || return 0
    name=$(timestamped_name "$1" "$2" pom <<<"$metadata") || return 0
    { curl -sf "$base_url/$name" || true; } | pom_commit
}

self_test() {
    local got
    got=$(pinned_commit <<<'source = "git+https://github.com/eclipse-zenoh/zenoh-flat-jni.git?branch=main#e75529ce3758401ce213456e7b8e4e5667635cf8"')
    [[ $got == e75529ce3758401ce213456e7b8e4e5667635cf8 ]] || { echo "pinned_commit: $got" >&2; exit 1; }

    # The real lockfile too, so a change to how Cargo writes it fails here
    # rather than by silently rebuilding on every run.
    got=$(pinned_commit <Cargo.lock)
    [[ $got =~ ^[0-9a-f]{40}$ ]] || { echo "pinned_commit(Cargo.lock): $got" >&2; exit 1; }

    got=$(timestamped_name zenoh-flat-jni 1.9.0-java-SNAPSHOT pom <<'EOF'
<versioning>
    <snapshot>
      <timestamp>20260810.012355</timestamp>
      <buildNumber>1</buildNumber>
    </snapshot>
</versioning>
EOF
    )
    [[ $got == zenoh-flat-jni-1.9.0-java-20260810.012355-1.pom ]] || { echo "timestamped_name: $got" >&2; exit 1; }

    # A release-style metadata carries no <snapshot> block: no name to build.
    if timestamped_name zenoh-flat-jni 1.9.0 pom <<<'<versioning><latest>1.9.0</latest></versioning>' >/dev/null; then
        echo "timestamped_name accepted metadata with no snapshot block" >&2
        exit 1
    fi

    got=$(pom_commit <<<'  <zenoh.flatJniCommit>e75529ce3758401ce213456e7b8e4e5667635cf8</zenoh.flatJniCommit>')
    [[ $got == e75529ce3758401ce213456e7b8e4e5667635cf8 ]] || { echo "pom_commit: $got" >&2; exit 1; }

    got=$(pom_commit <<<'<project><version>1.9.0-java-SNAPSHOT</version></project>')
    [[ -z $got ]] || { echo "pom_commit on an unstamped POM: $got" >&2; exit 1; }

    # A finished publication, then the two ways one can be unfinished: stopped
    # after the POM, and carrying a sources jar but no main jar. The layout is
    # what zenoh-flat-jni really publishes — checked against 1.9.0-rc8-SNAPSHOT.
    local finished="
      <snapshotVersion><extension>pom</extension></snapshotVersion>
      <snapshotVersion><extension>module</extension></snapshotVersion>
      <snapshotVersion><classifier>sources</classifier><extension>jar</extension></snapshotVersion>
      <snapshotVersion><extension>jar</extension></snapshotVersion>"
    advertises pom module jar <<<"$finished" || { echo "advertises: finished rejected" >&2; exit 1; }
    if advertises pom module aar <<<"$finished"; then
        echo "advertises: accepted a jar publication as an aar one" >&2; exit 1
    fi
    if advertises pom module jar <<<'<snapshotVersion><extension>pom</extension></snapshotVersion>'; then
        echo "advertises: accepted a publication that stopped after the POM" >&2; exit 1
    fi
    if advertises jar <<<'<snapshotVersion><classifier>sources</classifier><extension>jar</extension></snapshotVersion>'; then
        echo "advertises: took the sources jar for the main one" >&2; exit 1
    fi

    echo "flat-jni-copy.bash self-test OK"
}

main() {
    local version base commit rebuild=false entry artifact stamp
    version=$(sed -n 's/^zenohFlatJniVersion=//p' gradle.properties | tr -d '[:space:]')
    [[ $version == *-$qualifier-SNAPSHOT ]] || {
        echo "::error::zenohFlatJniVersion=$version is not a -$qualifier-SNAPSHOT copy;" \
             "only that coordinate is ours to publish" >&2
        exit 1
    }
    # zenoh-flat-jni derives the coordinate from its own version.txt, so it needs
    # to be told what we expect: a pin that moved past a version bump there would
    # otherwise publish 1.10.0-java-SNAPSHOT while we still resolve this.
    base=${version%-$qualifier-SNAPSHOT}
    commit=$(pinned_commit <Cargo.lock)

    for entry in "${artifacts[@]}"; do
        artifact=${entry%:*}
        stamp=$(published_commit "$artifact" "$version" "${entry#*:}")
        if [[ $stamp == "$commit" ]]; then
            echo "$artifact:$version is already $commit"
        else
            echo "$artifact:$version stamp is ${stamp:-none}, want $commit"
            rebuild=true
        fi
    done

    echo "commit=$commit base=$base rebuild=$rebuild"
    if [[ -n ${GITHUB_OUTPUT:-} ]]; then
        {
            echo "commit=$commit"
            echo "version=$version"
            echo "base=$base"
            echo "qualifier=$qualifier"
            echo "rebuild=$rebuild"
        } >>"$GITHUB_OUTPUT"
    fi
}

if [[ ${1:-} == --self-test ]]; then self_test; else main; fi
