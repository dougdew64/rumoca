# Reading Rumoca's Rust with contemporary C++

*Reference — look things up. Written for a reader who knows C++17/20/23 and wants to **read**
this workspace's Rust, not write it. Every C++ entry carries the standard version it arrived in
and the one point where the translation is loose, because a translation that is only evocative
teaches a slightly wrong C++ on the way to a slightly wrong Rust.*

*Anchors point at real declarations in this workspace, by symbol name rather than line number, so
they survive edits. Grep the symbol.*

---

## The one sentence

**Contemporary C++ and Rust are the same set of ideas, with the enforcement removed from one
side.** C++17, 20 and 23 adopted most of what Rust made mandatory: optional and expected values,
sum types, constrained generics, lazy range pipelines, ownership through smart pointers, modules.
What Rust adds is that the compiler *checks* the discipline C++ leaves to the reader. So if you
already think in `unique_ptr`, rvalue references and `std::variant`, the borrow checker is not a
new model — it is the model you use, with the mistakes turned into compile errors.

That makes the reading problem small. Almost every line of Rust in `crates/` and `hrw/src/`
transliterates into C++ you already know. The exceptions are listed in § 3, and they are exactly
where the reading effort belongs.

---

## 1. The translation table

Read the third column. It is the part a C++ developer cannot guess.

### Ownership and references

| Rust | contemporary C++ | the difference |
|---|---|---|
| `let x = …` / `let mut x = …` | `const auto x = …` / `auto x = …` | immutable is the **default**; `mut` is the annotation |
| `&T` / `&mut T` | `const T&` / `T&` | same model, plus one rule C++ has no analogue for: at any moment a value has **either** any number of `&T` **or** exactly one `&mut T`, never both. Most borrow-checker errors are this rule |
| passing `x` (a move) | `std::move(x)` (C++11) | Rust's move is **destructive**: `x` cannot be named again, and the compiler rejects the attempt. C++'s `std::move` is a cast; the moved-from object still exists in a valid-but-unspecified state, and using it is your mistake to find |
| `x.clone()` | copy construction | in Rust a copy is **always visible in the source** as `.clone()` (unless the type is `Copy` — ints, floats, `bool`, plain references). C++ copies implicitly on every by-value pass, which is the single biggest reading difference: in Rust, `f(x)` gives `x` away |
| `Box<T>` | `std::unique_ptr<T>` (C++11) | same: sole owner, heap, freed on drop |
| `Rc<T>` | `std::shared_ptr<T>` **with a non-atomic count** — which C++ does not have | `shared_ptr`'s reference count is always atomic. `Rc` is the cheaper single-threaded version, and the compiler **refuses to let it cross a thread** (it is not `Send`) |
| `Arc<T>` | `std::shared_ptr<T>` (C++11) | this one is exact: atomic count, safe to share across threads |
| `Arc<Mutex<T>>` | `shared_ptr<T>` + a `std::mutex` you remember to lock | Rust's `Mutex` **owns** the data; the only way to reach `T` is through the lock guard. Forgetting to lock is not expressible |
| `Drop` trait | destructor | same: RAII, deterministic, runs at scope exit in reverse declaration order |
| lifetimes `'a` | **none** | the one genuinely new thing. C++ has the dangling-reference problem; Rust has the annotation that prevents it. Tooling (`[[clang::lifetimebound]]`, the Core Guidelines lifetime profile) covers a corner of it. **For reading, skip past them**: `fn f<'a>(x: &'a T) -> &'a U` means "the result borrows from `x`", and that is all you need |
| `unsafe { … }` | every line of C++ | the block where the compiler's checks are suspended and the author vouches. See § 4 for how little of it this workspace has |

### Values that may be absent or may have failed

| Rust | contemporary C++ | the difference |
|---|---|---|
| `Option<T>` | `std::optional<T>` (C++17) | nearly exact. C++23 added the monadic `and_then` / `transform` / `or_else` that Rust's `Option` has had all along |
| `Result<T, E>` | `std::expected<T, E>` (C++23) | nearly exact; `Err(e)` is `std::unexpected(e)` |
| `?` | **nothing** | `let v = f()?;` is `auto r = f(); if (!r) return std::unexpected(r.error()); auto v = *r;` written by hand. It also works on `Option`. Read `?` as "unwrap, or return the failure to my caller" |
| `panic!` | `std::terminate` / `abort` | Rust has **no exceptions**. Errors travel as `Result` values; a panic is a bug, not a control-flow path. There is no `try`/`catch` to look for |
| `.unwrap()` / `.expect("…")` | `*opt` on an empty optional — but checked | panics if absent, with a message. In C++ the same access is undefined behaviour |
| `if let Some(x) = opt { … }` | `if (opt) { auto& x = *opt; … }` | Rust binds and tests in one line; there is also `let Some(x) = opt else { return … };` (the `let-else`, 2022) |

### Sum types and dispatch

| Rust | contemporary C++ | the difference |
|---|---|---|
| `enum E { A, B }` | `enum class E { A, B }` (C++11) | same |
| `enum E { A { x: i32 }, B(String) }` | `std::variant<A, B>` (C++17) with `A` and `B` as separate structs | Rust's is **first-class syntax**; `variant` is a library type. Same information, but Rust's alternatives have names and fields in one declaration |
| `match e { E::A { x } => …, E::B(s) => … }` | `std::visit(overloaded{ [](const A& a){…}, [](const B& b){…} }, e)` (C++17) | both are **exhaustive** — a `visit` whose callable cannot accept every alternative fails to compile, the same as a `match` with a missing arm. The differences are elsewhere: Rust **destructures** fields in the pattern (`{ x }`), C++ receives the whole alternative and reads members; Rust's arms are readable in place, C++ needs the `overloaded` helper and lambdas; and a generic `[](auto&&)` arm in C++ silently absorbs alternatives added later, exactly as `_ =>` does in Rust. **Language-level pattern matching did not make C++26**; it remains a proposal (P2688) |
| `if let E::A { x } = e { … }` | `if (auto* a = std::get_if<A>(&e)) { … }` (C++17) | one arm of a match, when that is all you need |
| `struct S(pub T);` — a *newtype* | `struct S { T value; };` written by hand | C++'s `using S = T;` is **not** this: an alias is the same type and a newtype is a distinct one. The point of the newtype is that `ParsedTree` and `ResolvedTree` cannot be passed for each other even though both wrap a `ClassTree`. Rust reaches the inner value as `s.0` |
| traits, statically dispatched | **concepts** (C++20) | the closest thing to a direct port, with one structural difference: a concept is satisfied by *any* type with the right shape (structural), a trait must be *implemented for* the type by name (`impl Trait for T`, nominal). The nominal rule is what gives Rust its coherence guarantee — one implementation per type per trait, anywhere in the program |
| `fn f(x: impl Trait)` or `fn f<T: Trait>(x: T)` | `template<Trait T> void f(T x)` (C++20) | identical mechanism: monomorphised per type. **One difference in when errors appear**: Rust type-checks the generic body once, against the bound, so a call that satisfies the bound cannot fail inside; C++ checks the constraint at the call but the body only at instantiation, which is where the long error messages come from |
| `-> impl Trait` | `auto` return type (C++14) constrained by a concept (C++20) | an opaque return type: the caller knows only that it satisfies the trait |
| `&dyn Trait` / `Box<dyn Trait>` | `const Base&` / `unique_ptr<Base>` with `virtual` | identical mechanism — a vtable — placed differently. C++ puts the vtable pointer **inside the object**; Rust puts it **in the reference**, which becomes a two-word "fat pointer". Consequence: a plain Rust struct with no virtual anything can be used through `dyn Trait`, because nothing in the struct's layout has to know |
| `impl Trait for Type { … }` | member functions defined out of class, or an ADL overload set | Rust can implement its own trait for a type it does not own (a foreign struct), which C++ can only approximate with free functions |
| `&self` / `&mut self` / `self` | `const` member function / non-`const` member function / `&&`-qualified member function (C++11 ref-qualifiers) | `self` by value **consumes** the object; it is how "builder" and "into" methods are written |
| `Self` | the injected class name | same |
| `Default` / `Clone` / `PartialEq` / `Debug` | default ctor / copy ctor / `operator==` / a formatter | the four you see most in `#[derive(…)]`. C++20 can `= default` `operator==` and `<=>`; `Debug` has no C++ analogue short of writing a `std::formatter` specialisation |

### Generics, iteration, closures

| Rust | contemporary C++ | the difference |
|---|---|---|
| `struct S<T: Trait>` | `template<Trait T> struct S` (C++20) | with concepts, the same |
| `Vec<T>` / `&[T]` / `[T; N]` | `std::vector<T>` / `std::span<T>` (C++20) / `std::array<T, N>` (C++11) | `&[T]` is a *slice*: pointer plus length, borrowing anything contiguous. `span` is exactly that |
| `String` / `&str` | `std::string` / `std::string_view` (C++17) | same pair: owning vs borrowed view. `&str` is guaranteed UTF-8 |
| `HashMap` / `BTreeMap` | `std::unordered_map` / `std::map` | same |
| `for x in v` / `for x in &v` / `for x in &mut v` | range-`for` by value / `const auto&` / `auto&` | the first **consumes** `v` (it is moved into the loop). The three forms are the ownership row again |
| `v.iter().filter(…).map(…).collect()` | `v \| views::filter(…) \| views::transform(…) \| ranges::to<vector>()` (C++20 views, C++23 `to`) | direct analogue; both lazy until the terminal step. `.count()`, `.sum()`, `.any()` are the algorithms at the end of the pipe |
| `\|x\| x + 1` / `move \|x\| …` | `[](auto x){ return x + 1; }` / `[y = std::move(y)](auto x){…}` (C++14 init-capture) | very close. Rust **infers** each capture's mode (by reference, by mutable reference, or by move) from how the body uses it; C++ makes you state it in the capture list. `move` forces by-value for all captures, which is what you see on closures handed to threads |
| `Iterator` trait / `IntoIterator` | input range / `begin()`+`end()` | same idea: a type that can be stepped through |

### Modules, visibility, compile-time

| Rust | contemporary C++ | the difference |
|---|---|---|
| crate | a library / a named module (C++20) | the unit of compilation **and** of visibility. `crates/rumoca-compile` is one crate; `hrw` is one |
| `mod name` | `namespace name` — but also a **privacy boundary** | a C++ namespace controls names only; anything in it is reachable. A Rust module is where `private` lives: an item without `pub` is visible in its module and the modules nested inside it, and nowhere else |
| `pub` / `pub(crate)` / (nothing) | `export` (C++20 modules) / not exported / `static` or anonymous namespace | `pub(crate)` ≈ visible throughout the crate, invisible to anyone who links it: what a C++20 module's non-`export`ed declarations are to an importer |
| `use a::b::C;` | `using a::b::C;` | same |
| `#[cfg(feature = "x")]` | `#ifdef X` | same job — the item is removed before type-checking — but `cfg` is applied after parsing, so the removed code must still be syntactically valid Rust. `if constexpr` (C++17) is the *other* thing: a branch inside a template body, discarded per instantiation |
| `#[derive(Debug, Clone, Serialize)]` | `= default` for the few C++ can default; **reflection (C++26)** for the rest | `derive` hands you a whole implementation. C++26 reflection (P2996) lets you *write* a generic `operator==` or printer once by walking members, which covers the same ground; the compiler does not hand it to you. Rust's `derive` is a *procedural macro*: a Rust function that runs at compile time over the struct's tokens and emits the `impl` — `serde_derive` is the one behind `Serialize`, and it is what makes `Stage::from_ser` work for every IR type without any of them knowing |
| `macro_rules!` / `foo!(…)` | preprocessor macros | Rust's operate on **token trees** and are hygienic (a name introduced inside cannot capture or be captured by a name outside). C++'s are text substitution. `format!`, `vec!`, `println!` are the ones you will see; read the `!` as "this is a macro, its arguments may not be ordinary expressions" |
| `const` / `static` | `constexpr` / a global | `const` items are inlined at every use; `static` has one address |

---

## 2. Where to see each one in this workspace

Grep the symbol. Each is a short declaration that carries the idiom in a form you can read whole.

| idiom | symbol | file | what to notice |
|---|---|---|---|
| newtype | `pub struct ParsedTree(pub ClassTree);` and `ResolvedTree` | [`rumoca-ir-ast/src/lib.rs`](../../crates/rumoca-ir-ast/src/lib.rs) | two distinct types over one representation, so a phase cannot be handed the wrong one. The doc comment above `ParsedTree` lists what is *not yet filled in* at that stage |
| lifetime + `dyn` + closure | `pub type FrameObserver<'a, F> = &'a dyn Fn(&F);` | [`rumoca-core/src/lib.rs`](../../crates/rumoca-core/src/lib.rs) | one line, three rows: a borrowed (`&'a`) reference to a type-erased (`dyn`) callable (`Fn`) taking a frame by reference. In C++: `const std::function<void(const F&)>&`. The `'a` says the observer must outlive the phase it is handed to; read past it |
| enum with data | `pub enum Expression` | [`rumoca-core/src/ir_primitives.rs`](../../crates/rumoca-core/src/ir_primitives.rs) | **the** idiom of the IR. `Binary { op, lhs: Box<Expression>, rhs: Box<Expression>, span }`, `BuiltinCall { function, args, span }`, … The `Box` is the C++ `unique_ptr` a recursive tree needs; the `#[serde(…)]` attributes steer the derived serializer per field |
| exhaustive `match` on it | `expr_format`'s main `match expr` | [`hrw/src/expr_format.rs`](../src/expr_format.rs) | twenty `Expression::` arms and no `_ =>`, deliberately: adding a variant to the enum breaks this function's build, which is the point |
| `if let` on one arm | `if let Expression::Binary { op: child_op, .. } = child` | same file | the `..` is "ignore the other fields" |
| generic bound + `match` on `Result` + macro | `fn from_ser<T: serde::Serialize>(v: &T) -> Self` | [`hrw/src/worker.rs`](../src/worker.rs) | five lines that use six rows. See § 3 |
| `?` | `let zero = self.emit_const_at(0.0, span)?;` | [`rumoca-phase-solve/src/lower/array_values/builtins.rs`](../../crates/rumoca-phase-solve/src/lower/array_values/builtins.rs) | every `?` in that function is an early return of the error to the caller; the function's own `-> Result<…>` is what permits it |
| `pub(crate)` | `pub(crate) fn record_cache_file_access` | [`rumoca-compile/src/cache.rs`](../../crates/rumoca-compile/src/cache.rs) | callable from anywhere inside `rumoca-compile`, invisible to `hrw` |
| `Arc<Mutex<…>>` | `type CopySink = std::sync::Arc<std::sync::Mutex<Option<String>>>;` | [`hrw/src/app.rs`](../src/app.rs) | shared across threads, data reachable only through the lock |
| `Arc<AtomicBool>` | `live_done: Arc<AtomicBool>` | [`hrw/src/playback.rs`](../src/playback.rs) | the C++ `std::atomic<bool>` behind a `shared_ptr` |
| `#[cfg(feature = …)]` | `#[cfg(feature = "console_error_panic_hook")]` | [`rumoca-bind-wasm/src/lib.rs`](../../crates/rumoca-bind-wasm/src/lib.rs) | a Cargo *feature* is a named build flag; the item exists only when the feature is on |
| `unsafe` at an FFI edge | `unsafe fn create_pipe` | [`hrw/src/worker.rs`](../src/worker.rs) | the doc comment above it says why: `libc::pipe` is a C function, and calling C is always `unsafe` because the Rust compiler cannot see its contract |

---

## 3. The four places the effort goes

Everything else transliterates. These four do not, and they are in rough order of how often you
will meet them in this workspace.

### Enum-with-data plus `match` — you cannot read past this one

`std::variant` gives you the concept, but Rumoca's IR is *written* in this idiom: `Expression` and
`Statement` in `rumoca-core`, the AST's `Equation` (an enum there; by the DAE stage it has
become a plain struct), the outcome types, the frame types the observers receive. Reading it
fluently is the skill. The habits that make it read:

- **A `match` is an expression.** `let s = match e { … };` assigns whichever arm ran. Arms are
  `pattern => expression,` and a block `{ … }` is an expression whose value is its last line
  without a semicolon.
- **The pattern does the destructuring.** `Expression::Binary { op, lhs, rhs, .. }` binds three
  locals named after the fields and ignores the rest. There is no `std::get`.
- **No `_ =>` arm means the compiler is checking exhaustiveness for you**, and a missing arm is
  the build error that says a variant was added. When you see `_ =>`, ask what it is absorbing.
- **`if let` is a `match` with one arm you care about.**

The worked line: `Stage::from_ser` in `hrw/src/worker.rs`.

```rust
fn from_ser<T: serde::Serialize>(v: &T) -> Self {
    match serde_json::to_value(v) {
        Ok(val) => Stage::ok(val),
        Err(e) => Stage::err(format!("serialization failed: {e}")),
    }
}
```

In C++23 terms: `template<Serializable T> static Stage from_ser(const T& v)` whose body calls
something returning `std::expected<Value, Error>` and visits both alternatives. What the Rust
says, row by row: a generic function constrained by a trait (concept); taking a borrow (`const&`);
returning `Self` (the enclosing type); calling a function that returns `Result` (`expected`);
matching its two alternatives exhaustively, binding the payload of each (`val`, `e`); and in the
error arm, a macro (`format!`) with an inline capture of `e` in the format string. Six rows, five
lines, and nothing that is not in the table.

### Ownership — the same model, enforced

If `unique_ptr` and rvalue references are how you already think, then two adjustments make the
borrow checker legible:

1. **A move ends the name.** After `let b = a;` (for a non-`Copy` type), `a` is gone. The compiler
   says so. There is no moved-from state to reason about.
2. **A copy is spelled out.** `.clone()`. If you do not see it, no copy happened — the value moved,
   or a reference was taken.

The exclusivity rule (`&mut` is alone) is the one with no C++ precedent, and it is why code that
looks like it should work sometimes has an extra scope, a `clone()`, or a value pulled out of a
struct before a loop. Those are the author appeasing the rule, not doing something clever.

### `Result` and `?` — there are no exceptions

There is no `try`/`catch` anywhere and nothing that throws. Every fallible function says so in
its return type, every caller either handles the `Err` or forwards it with `?`, and the shape of
a function's error handling is visible in its signature. This is also why the compiler is a
*recovering* compiler: problems in a model are values (flags, outcomes) that flow forward, not
exceptions that unwind. `panic!` and `.unwrap()` are the only way out, and they mean "this is a
bug", not "this is an error".

### Lifetimes — read past them

`'a` annotations answer one question: *which input does this output borrow from?* For reading
Rumoca, you almost never need the answer; you need to know that a `&'a T` is a reference that
does not outlive something, and that is enough. The observer type above is the only place in the
instrumentation where one appears, and it says the observer outlives the phase.

---

## 4. How much `unsafe` there is, and where

A C++ reader expects `unsafe` to be everywhere. It is not. Across `crates/*/src` there are 72
occurrences, and they cluster at the edges where Rust meets something it cannot see the contract
of: the CUDA driver (`rumoca-exec-mlir/src/cuda_driver.rs`, 31), Cranelift code emission
(`rumoca-exec-cranelift/src/emit.rs`, 12), and the MLIR compile path. In the phases you will
actually read — parse through solve, `rumoca-compile`, `rumoca-core` — it appears in **three
files**, all in `rumoca-phase-structural`'s `dae_prepare/`. HRW's own eighteen are all one thing:
the `libc` pipe and `dup2` calls that capture a compile's stderr in `worker.rs`.

So the reading stance is: the compiler's guarantees hold on every line you are likely to be
looking at, and where they are suspended, a doc comment says why.

---

## 5. What was checked against the standard, and what to distrust

The C++ version tags are as of the C++26 draft that was technically complete in mid-2025:
`std::optional` and `std::variant` (17); `std::string_view` (17); concepts, ranges, modules,
`std::span` (20); `std::expected`, `std::ranges::to`, monadic `optional` (23); reflection
(P2996, 26). Two things a reader might expect and should not: **language-level pattern matching
is not in C++26** (P2688 remains a proposal), and **C++ has no non-atomic `shared_ptr`** — `Rc`
has no direct counterpart.

Two entries in this table correct an earlier verbal version of it: `std::visit` *is* exhaustive
(the earlier account said it was not), and reflection is *in* C++26 (the earlier account said
proposed). If you find a third, the fix goes here and in the ledger, on the usual terms.
