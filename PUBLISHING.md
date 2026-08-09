# Publishing zenoh-java

This document describes how `zenoh-java` is built, verified, and published to
Maven Central, and how to rehearse a release without publishing one.

It describes the pipeline as it exists in this repository. Where something is
not yet implemented or not yet exercised, it is listed under
[Known gaps](#known-gaps) rather than described as if it worked.

If you just need to run a release, go to [Running a release](#running-a-release).
For the JVM publishing concepts — coordinates, staging, signing — see
[zenoh-flat-jni's PUBLISHING.md](https://github.com/eclipse-zenoh/zenoh-flat-jni/blob/main/PUBLISHING.md#background-if-you-do-not-work-in-the-jvm-ecosystem),
which covers them once for both repositories.

## Contents

- [What this repository publishes](#what-this-repository-publishes)
- [Relationship to zenoh-flat-jni](#relationship-to-zenoh-flat-jni)
- [Running a release](#running-a-release)
  - [Before the first run](#before-the-first-run)
  - [Rehearsal (dry run)](#rehearsal-dry-run)
  - [The real release](#the-real-release)
  - [After a release](#after-a-release)
- [Rehearsing before zenoh-flat-jni is released](#rehearsing-before-zenoh-flat-jni-is-released)
- [How the pipeline works](#how-the-pipeline-works)
- [Local development](#local-development)
- [Required secrets](#required-secrets)
- [Known gaps](#known-gaps)
- [Release checklist](#release-checklist)

## What this repository publishes

```text
org.eclipse.zenoh:zenoh-java:<version>          the JVM artifact
org.eclipse.zenoh:zenoh-java-android:<version>  the Android artifact
```

Both are **pure JVM/Kotlin**. This repository contains no Rust and builds no
native libraries: they arrive inside
`org.eclipse.zenoh:zenoh-flat-jni:<version>`, already cross-compiled for six
desktop targets and four Android ABIs, and verified by that repository's own
release. A consumer of `zenoh-java` gets them transitively.

That is the whole reason the publishing here is simple — there is no build
matrix, no cross-compilation, and no native artifact to inspect.

## Relationship to zenoh-flat-jni

```text
zenoh-flat-jni:<version> released to Maven Central
                  |
                  v
        zenoh-java built against it
                  |
                  v
        zenoh-java:<version> released
```

**The order is not a convention, it is a constraint.** `zenoh-java` cannot be
released until the `zenoh-flat-jni` version it depends on is really on Maven
Central, because a release must not depend on a snapshot:

- consumers do not have the Central *snapshot* repository configured, so the
  dependency would simply fail to resolve for them;
- snapshots are mutable and are eventually removed, so even where it resolved it
  would not stay reproducible.

`ci/scripts/bump-and-tag.bash` refuses a `-SNAPSHOT` value outright rather than
letting that reach a published POM.

Rehearsals are not constrained this way — see
[Rehearsing before zenoh-flat-jni is released](#rehearsing-before-zenoh-flat-jni-is-released).

## Running a release

Everything is driven from **Actions → Release → Run workflow** on the default
branch. The workflow creates the release branch, bumps the version, tags it,
builds and publishes.

### Before the first run

- **Secrets are already in place.** `CENTRAL_SONATYPE_TOKEN_*` and `ORG_GPG_*`
  are organization-level secrets on `eclipse-zenoh`, inherited automatically.
- **The zenoh-flat-jni version must already be on Maven Central** for a live
  run. Check before starting:

  ```bash
  curl -sfI https://repo1.maven.org/maven2/org/eclipse/zenoh/zenoh-flat-jni/<version>/zenoh-flat-jni-<version>.pom
  ```

### Rehearsal (dry run)

| Field | Value |
| --- | --- |
| `live-run` | **unchecked** |
| `version` | a fresh provisional number, not one already used |
| `zenoh-flat-jni-version` | leave empty for a rehearsal, or a released version |
| `maven_publish` | checked — or uncheck for the very first run |

`live-run` and `maven_publish` behave exactly as in zenoh-flat-jni: unchecking
`live-run` publishes `<version>-SNAPSHOT` to the **mutable** snapshot repository
and never runs `closeAndReleaseSonatypeStagingRepository`, while `maven_publish`
decides whether any upload happens at all. **A rehearsal with `maven_publish`
checked performs a real, signed upload** — into the snapshot repository — and is
the only configuration that exercises the credentials.

`bump-and-tag.bash` tags whatever version it is handed, rehearsals included, so
never give a rehearsal the number you intend to release.

### The real release

| Field | Value |
| --- | --- |
| `live-run` | **checked** |
| `version` | the release number |
| `zenoh-flat-jni-version` | the zenoh-flat-jni release to build against — **must already be on Central** |
| `maven_publish` | checked |

Supplying `zenoh-flat-jni-version` rewrites `zenohFlatJniVersion` in
`gradle.properties` and commits it, so the published POM records exactly which
binding release the SDK was built against.

### After a release

Confirm the coordinates resolve, then verify the dependency is right — the POM
must reference a real `zenoh-flat-jni` release:

```bash
curl -s https://repo1.maven.org/maven2/org/eclipse/zenoh/zenoh-java/<version>/zenoh-java-<version>.pom \
  | grep -A2 zenoh-flat-jni
```

## Rehearsing before zenoh-flat-jni is released

This is the common case during the transition, and it works — only the *live*
release is blocked.

| Rehearsal | resolves zenoh-flat-jni from | proves |
| --- | --- | --- |
| local build and tests | a sibling checkout, via `-PuseLocalFlatJni=true` | the code compiles and the tests pass |
| CI, `maven_publish` unchecked | the snapshot repository, or the sibling in CI | the artifact assembles |
| CI, snapshot publication | `zenoh-flat-jni:<version>-SNAPSHOT` | signing, credentials, a real upload |
| live release | `zenoh-flat-jni:<version>` on Central | **blocked until that exists** |

A snapshot may depend on a snapshot, because nothing published is permanent. So
to rehearse against an unreleased binding, point the build at the snapshot
zenoh-flat-jni's own rehearsal published:

```bash
./gradlew build -PzenohFlatJniVersion=1.9.0-rc3-SNAPSHOT
```

For that to resolve, the snapshot repository must be declared in the build's
`repositories` block. It is **not** declared by default, deliberately: a
repository that can serve mutable artifacts should not be in the resolution path
of a release. Add it only for the duration of a rehearsal.

The alternative, and the one to prefer while developing, is not to involve a
repository at all — see [Local development](#local-development).

## How the pipeline works

`release.yml` runs three jobs:

1. **`tag`** — `eclipse-zenoh/ci/create-release-branch` cuts the release branch,
   then `ci/scripts/bump-and-tag.bash` writes `version.txt`, optionally rewrites
   `zenohFlatJniVersion` in `gradle.properties`, commits and tags. It refuses a
   `-SNAPSHOT` binding version.
2. **`publish-jvm`** and **`publish-android`** — compile Kotlin and publish. No
   native toolchain is installed and no Rust is built; both would be pointless
   here.
3. **`publish-github`** — creates the GitHub release, on a live run only.

Publishing goes through `io.github.gradle-nexus.publish-plugin` to the Central
Portal, signed with the organization GPG key, exactly as in zenoh-flat-jni.

## Local development

The default build resolves `zenoh-flat-jni` from Maven Central like any consumer.
To work against a local checkout of it:

```bash
./gradlew build -PuseLocalFlatJni=true
```

That substitutes `../zenoh-flat-jni` through a Gradle composite build, so changes
there are picked up without publishing anything. Set it in your personal
`gradle.properties` if you want it always on.

**It must never be enabled for a release.** With it on, the published artifact
would be built against whatever happens to be on the builder's disk rather than
the resolved dependency, and the POM would still claim the released version. It
is off by default and CI passes it explicitly only where it checks the sibling
out.

## Required secrets

| Secret | Use |
| --- | --- |
| `CENTRAL_SONATYPE_TOKEN_USERNAME` / `_PASSWORD` | Central Portal user token |
| `ORG_GPG_KEY_ID` / `_SUBKEY_ID` / `_PRIVATE_KEY` / `_PASSPHRASE` | signing |
| `BOT_TOKEN_WORKFLOW` | release branch, tag push, GitHub release |

All are organization-level on `eclipse-zenoh`; nothing is configured per
repository.

## Known gaps

- **The rewritten release path has never run.** The workflows were repaired for
  a repository that no longer contains Rust; no rehearsal has yet exercised
  them.
- **No consumer test.** Unlike zenoh-flat-jni, nothing resolves the published
  `zenoh-java` artifact from a repository and runs it before release. The tests
  here run against the build's own output.
- **The Android artifact has no runtime test**, and its `ndkVersion` and NDK
  setup step are retained although no native code is built here — unverified
  whether the Android Gradle Plugin still needs them.
- **`zenoh-flat-jni` itself has not been released**, so the ordering constraint
  above has never been satisfied for a real release.

## Release checklist

- [ ] The `zenoh-flat-jni` version to build against is on Maven Central.
- [ ] `version.txt` and the intended tag agree.
- [ ] A rehearsal completed under a fresh version, with publication enabled at
      least once so signing and credentials were exercised.
- [ ] `useLocalFlatJni` is off — the release resolves from Central.
- [ ] The published POM references a released `zenoh-flat-jni`, not a snapshot.
- [ ] The released coordinates resolve from Maven Central.
