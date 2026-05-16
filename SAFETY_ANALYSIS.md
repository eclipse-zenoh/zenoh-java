# Safety analysis: `opaque_arc_input` / `opaque_arc_output`

Analysis of the Rust ↔ Java/Kotlin opaque-handle lifecycle on the
`wrapper_update` branch, focused on memory safety and interaction with the
JVM garbage collector.

## 1. The mechanism

**Wire form.** A `jlong` carries `Arc::into_raw(Arc::new(v))`. `0` is reserved
as the `None` niche (`Arc::into_raw` never returns 0).

**Output** (`prebindgen-ext/src/jni/jni_ext.rs:272`): on every Rust → Java
handoff, wrap `T` in a fresh `Arc`, leak its pointer to Java. Refcount = 1
sitting in Java.

**Input** (`prebindgen-ext/src/jni/jni_ext.rs:233`): the converter reconstructs
the Arc via `OwnedObject::from_raw` and dispatches on parameter kind:

- `&T` parameter — `OwnedObject` derefs to `&T`; `Drop` runs `mem::forget` so
  Java's strong count is untouched (borrow).
- `T` by-value parameter — call site emits `.consume()?`, which is
  `Arc::try_unwrap`. Success drops Java's strong count to zero and moves `T`
  out; failure returns the wrapper, whose `Drop` then `mem::forget`s the Arc
  back into the pointer Java holds (`prebindgen-ext/src/jni/jni_ext.rs:1810-1822`,
  `1847-1850`).

Net effect: **by-value transfer is itself the destruction signal**, no
separate destructor entry point needed — except for types that have no
by-value Rust function (`Publisher`, `Subscriber`, `KeyExpr`, …), which get
hand-written `freePtrViaJNI` / `dropKeyExprViaJNI` doing `Arc::from_raw(ptr)`
plus implicit drop (`zenoh-jni/src/publisher.rs:36`, `zenoh-jni/src/key_expr.rs:84`).

## 2. What's correct

- **ABI**: `jlong` is the right wire (matches 32-bit, where `*const T` is
  narrower than `jlong`). `0` niche is sound for `Option`.
- **`Arc::into_raw` / `from_raw` round-trip**: Rust guarantees this is stable
  for the same `T` in the same allocator. Both sides are in `zenoh-jni`, so
  fine.
- **Consume failure is safe**: when `try_unwrap` fails, `OwnedObject` rebuilds
  itself around the Arc and `Drop` `mem::forget`s — refcount is preserved,
  Java's pointer still valid. No double-free.
- **Niche on `Option<T>`**: `*v == 0 → None` matches the legacy null-pointer ABI.
- **`Send + Sync`**: enforced statically via `Arc<T>` — the compiler refuses
  to publish a non-`Send`/`Sync` handle.
- **Wrapper `Drop` is panic-safe**: even if a Rust call panics mid-way,
  `OwnedObject::drop` runs and forgets the temporary Arc, leaving Java's
  refcount intact.

## 3. Concerns with Java's garbage collector

### 3.1 `finalize()` is fragile (real but limited problem)

`Session`, `Publisher`, `Scout`, `Config` use `protected fun finalize()` as a
backstop calling `close()` (`zenoh-java/.../Session.kt:122`, `Publisher.kt:126`).
Issues:

- **No timeliness guarantee**: the JVM may never run finalizers, especially on
  shutdown. Network resources (zenoh undeclares, sessions) can leak past
  process exit.
- **`finalize` is deprecated** in modern JVMs (Java 9+); `java.lang.ref.Cleaner`
  or `PhantomReference` is the recommended replacement.
- **Finalizer thread races user threads**: `finalize` calling
  `jniPublisher?.close()` with no memory barrier on the field can see stale
  state. The field is `var jniPublisher: JNIPublisher?` without `@Volatile`.

### 3.2 The real correctness bug — concurrent borrow + consume is UB

This is the biggest issue and it's structural, not a finalizer issue:

`OwnedObject::from_raw` **does not increment the refcount** — it just
reconstructs an Arc handle aliasing Java's leaked one. The borrow path then
`mem::forget`s. That's correct in isolation, but it means *two* concurrent
calls into Rust on the same handle each call `Arc::from_raw(same_ptr)`,
producing two `Arc` instances that share a single logical refcount of 1.

If thread A is mid-call inside `&Session` borrow code while thread B calls
`dropSessionViaJNI` (consume) or `freePtrViaJNI`:

1. B's `Arc::from_raw` + `try_unwrap` sees refcount = 1 → succeeds → frees
   `ArcInner`.
2. A is still dereferencing the same `ArcInner` → **use-after-free**.

`Arc`'s atomic refcounting only protects against races between `clone` /
`drop` of *separate* `Arc` instances created through `clone`. Repeated
`from_raw` of the same pointer doesn't go through that invariant.

The high-level Kotlin classes mitigate this by nulling out `jniPublisher` /
`jniSession` on close, but there's no synchronization:

- `Session.close()` and `Publisher.close()` set the field to `null` *after*
  the JNI call returns.
- No lock, no `@Volatile`, no atomic. Thread A reading `jniPublisher`
  (non-null) and calling `put`, while thread B calls `close` → race.
- `finalize()` thread vs. user thread has the same race.

The Session `close()` does `closeSessionViaJNI` (borrow) then
`dropSessionViaJNI` (consume) sequentially on one thread
(`JNISession.kt:243`) — fine in isolation, but unsafe if any other thread is
mid-call.

### 3.3 Consume can silently leak on internal clones

If zenoh internally clones the `Arc` (for example, tasks holding
`Arc<Session>`), `try_unwrap` fails, `?` returns the error to Java as
`ZError`. Java's pointer remains valid (good), **but the close path stops** —
the Kotlin code propagates the exception out of `close()`, leaving
`jniSession` non-null. Whether retry happens depends on user code.
Documented in the wrapper as a "JVM-side contract bug" but really it's a
zenoh-runtime question.

### 3.4 Minor: raw `Long` handle in `JNI*.kt` is not nulled on close

`JNIPublisher.ptr` is a `final val Long`. After `close()` runs
`freePtrViaJNI`, `ptr` is dangling. The class itself doesn't track this —
only the outer wrapper's nullable reference does. If anyone retains a
`JNIPublisher` reference past close (escape from the wrapper), subsequent
calls into native are use-after-free.

## 4. Summary

| Aspect | Verdict |
|---|---|
| ABI & layout | Correct |
| Single-threaded lifecycle | Correct |
| Consume failure path | Correct (no double-free, no use-after-free) |
| `OwnedObject::Drop` semantics | Correct |
| Concurrent borrow + consume/free on same handle | **Unsound** — UAF possible |
| `finalize()` as cleanup backstop | Works but fragile/deprecated |
| Wrapper field synchronization (`var jniSession: JNISession?`) | Missing |

The Arc-handle design is sound for the **single-threaded** owner-on-Java side
use case. The hole is concurrent access: nothing in the JNI layer or the
Kotlin wrappers prevents one thread from freeing a handle while another is
calling into it. Fixing this needs either (a) `@Volatile` + an
`AtomicReference`-style compare-and-null on the Kotlin side combined with
making the Rust borrow path do `Arc::clone` (proper refcount bump) instead of
`from_raw` + `forget`, or (b) a `RwLock`/refcount on the Kotlin side that
gates JNI entry against close. Option (a) is the more idiomatic fix — it
would also align `OwnedObject` with Arc's actual contract.

`finalize()` should migrate to `java.lang.ref.Cleaner` regardless; it's
orthogonal to the race bug but worth doing.

## 5. Comparison with the hand-written JNI layer on `main`

The `main` branch implements the same idiom by hand. Every JNI op follows one
of two patterns:

**Borrow** (`zenoh-jni/src/publisher.rs` `putViaJNI` etc.):

```rust
let publisher = Arc::from_raw(publisher_ptr);
// ... do work via &publisher ...
std::mem::forget(publisher);
```

**Free** (`closeSessionViaJNI`, `freePtrViaJNI`):

```rust
Arc::from_raw(session_ptr);  // reconstruct + let drop
```

The Kotlin side is identical in spirit: `JNIPublisher(private val ptr: Long)`
+ `freePtrViaJNI(ptr)` on close; `Session` exposes a `var jniSession: JNISession?`
that `close()` nulls and `finalize()` re-calls.

### Mapping the two designs

| Concept | `main` (hand-written) | `wrapper_update` (`opaque_arc_*`) |
|---|---|---|
| Wire | `*const T` (or `jlong` via Kotlin `Long`) | `jni::sys::jlong` |
| Output | `Arc::into_raw(Arc::new(v))` | `Arc::into_raw(Arc::new(v)) as i64` |
| Borrow path | `let x = Arc::from_raw(p); ...; mem::forget(x);` | `OwnedObject::from_raw(p)` → `Deref` → `Drop` `mem::forget`s |
| Free path | `Arc::from_raw(p);` (drop unconditionally) | Two flavours: `freePtrViaJNI` (same as main) **or** by-value param → `OwnedObject::consume()` → `Arc::try_unwrap` |
| `Option<T>` / nullable | Manual `if !ptr.is_null()` checks | Generated niche `*v == 0 → None` |
| Kotlin lifecycle | `JNI*.kt` holds `Long`, outer wrapper nulls a `var jniX: JNI*?` | Identical |
| `finalize()` backstop | Yes, on `Session`, `Publisher`, … | Yes, identical |

### Which bugs apply to `main`?

- **3.1 `finalize()` fragility — same on `main`.** Verbatim same code:
  `protected fun finalize() { close() }`. Same deprecation, same lack of
  timeliness, same finalize/user-thread race.

- **3.2 Concurrent borrow + free UAF — same on `main`, arguably starker.**
  On `main`, `put` does `Arc::from_raw` + `mem::forget`, and `freePtrViaJNI`
  does `Arc::from_raw` + drop. If thread A is between `from_raw` and
  `mem::forget` in `put` while thread B calls `freePtrViaJNI`, two `Arc`s
  alias the same allocation with refcount = 1. B's drop frees the `ArcInner`;
  A's deref is UAF. **Identical race**. `wrapper_update` inherits this exactly
  — `OwnedObject::from_raw` is the same `Arc::from_raw`, `Drop` does the
  same `mem::forget`.

- **3.3 Internal-clone behaviour — different shape, `main` is more lenient.**
  `main`'s close does `Arc::from_raw(p);` (decrement, possibly drop later);
  `wrapper_update`'s by-value `consume` uses `try_unwrap` and **leaks**
  Java's Arc back if any internal clone exists. `main` instead lazily defers
  the drop until the last internal clone goes away. The `wrapper_update`
  `freePtrViaJNI` paths (Publisher/Subscriber/KeyExpr) match `main` exactly,
  so this difference applies only to types with a `fn drop_x(x: T)` in
  `zenoh-flat` (`drop_session`, `drop_config`).

- **3.4 `JNIPublisher.ptr` not nulled — same on `main`.** Same pattern:
  `private val ptr: Long`, set once, never zeroed.

- **Wrapper-field synchronization — same on `main`.**
  `internal var jniSession: JNISession? = null`,
  `private var jniPublisher: JNIPublisher?`. Neither `@Volatile` nor
  `synchronized`. Same race window.

### Differences that are *not* safety differences

- **Code-size / consistency**: `wrapper_update` collapses ~15 hand-written
  `Arc::from_raw … mem::forget` boilerplate sites into one generic
  `OwnedObject`. Less surface for typos like "forgot `mem::forget`" — a real
  category of bug the generated version structurally avoids.
- **Niche encoding**: `wrapper_update`'s `0i64 ⇒ None` is automated. `main`
  has manual `if !ptr.is_null()` scattered everywhere; same outcome, more
  inconsistency risk.
- **By-value parameter support**: `wrapper_update` adds `consume`; `main` has
  no equivalent (every destructive op is a separate `freePtrViaJNI`).

### Verdict

Both designs have the same memory-model bugs:

1. Cross-thread borrow vs. free is UB.
2. `finalize` is deprecated and unreliable.
3. Wrapper-field access is not synchronized.

The `wrapper_update` design is **not less safe than `main`** — it's the same
model, automated. It mildly *reduces* a class of human errors (forgotten
`mem::forget`, inconsistent null handling). It mildly *increases*
leak-on-drop risk when internal Arc clones exist (because `try_unwrap` is
stricter than letting an `Arc` drop). Neither effect touches the central UAF
race; fixing that requires changes orthogonal to the input/output convention
— proper `Arc::clone` on the borrow path and proper synchronization on the
Kotlin field.
