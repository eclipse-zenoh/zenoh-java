# zenoh-flat transition

This branch (`zenoh-flat-transition`) is the **integration branch** for rebuilding
zenoh-java on top of the generated JNI/Kotlin bindings, replacing the hand-written
`zenoh-jni` layer. It exists so the transition can land as a series of reviewable
PRs targeting this branch instead of `main`; when the transition is complete, this
branch merges to `main` as a whole (and this file is removed).

## Architecture

```
zenoh (Rust)
  └─ zenoh-flat            flat #[prebindgen]-annotated Rust API
       └─ zenoh-flat-jni   generated JNI externs + Kotlin classes (prebindgen lang::JniGen)
            └─ zenoh-java  Kotlin SDK wrapper (this repo)
```

- **prebindgen** — <https://github.com/milyin/prebindgen> (generator)
- **zenoh-flat** — <https://github.com/ZettaScaleLabs/zenoh-flat> (flat Rust API)
- **zenoh-flat-jni** — <https://github.com/ZettaScaleLabs/zenoh-flat-jni> (generated bindings,
  consumed as a sibling checkout in CI and via Gradle composite build locally;
  as a Maven artifact once published)

## Constituent PRs

| PR | Scope | Status |
| --- | --- | --- |
| [#481](https://github.com/eclipse-zenoh/zenoh-java/pull/481) | Use receiver-style zenoh-flat-jni bindings (generated Session/Query/Publisher methods, split key-expr overloads, de-prefixed callback names); +61–83% subscriber throughput | merged |
| [#484](https://github.com/eclipse-zenoh/zenoh-java/pull/484) | `Encoding`/`ZenohId.toString` become pure JVM values (fixes a per-message native leak); the conversion logic lives in the **shared tier** (`EncodingCodec`/`zidString` in zenoh-flat-jni, correspondence-tested against native, reusable by zenoh-kotlin) with only a thin facade here; publisher **default encoding set natively at declare** (plain puts cross no encoding data). Pairs with [zenoh-flat-jni#4](https://github.com/ZettaScaleLabs/zenoh-flat-jni/pull/4), [zenoh-flat#3](https://github.com/ZettaScaleLabs/zenoh-flat/pull/3) (merged), [prebindgen#80](https://github.com/milyin/prebindgen/pull/80) (merged) | in progress |
| [#491](https://github.com/eclipse-zenoh/zenoh-java/pull/491) | `Session.undeclare` detaches the key-expr handle even when the native undeclare fails (the generated wrapper consumes it either way); found on the zenoh-kotlin port ([zenoh-kotlin#668](https://github.com/eclipse-zenoh/zenoh-kotlin/pull/668)) | merged |
| shared-parameters | `Parameters` becomes a thin facade over the shared string-backed implementation in zenoh-flat-jni (Rust `parameters.rs` semantics: no percent-decoding, infallible parse, first-match-wins get) + `ParametersCorrespondenceTest` vs the native oracle. Pairs with [zenoh-flat#4](https://github.com/ZettaScaleLabs/zenoh-flat/pull/4), [zenoh-flat-jni#9](https://github.com/ZettaScaleLabs/zenoh-flat-jni/pull/9) | open |

Companion PRs in the upstream repos are coordinated per constituent PR (e.g.
[ZettaScaleLabs/zenoh-flat-jni#3](https://github.com/ZettaScaleLabs/zenoh-flat-jni/pull/3)
pairs with #481 and merges first; CI here pins the exact upstream commits).

## CI pinning

`.github/workflows/ci.yml` on the constituent branches pins the exact
`zenoh-flat-jni` / `zenoh-flat` commits the code was written against, while
`prebindgen` resolves from its `main`. Pins are bumped as the upstream PRs land.
