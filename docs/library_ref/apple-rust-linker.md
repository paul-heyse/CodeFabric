# Auditing and Optimizing the Apple Linker for Rust on macOS

## Purpose

This guide provides a systematic procedure for determining:

1. Which linker your Rust builds are actually using.
2. Whether Rust/Cargo is reaching Apple's current linker correctly.
3. Whether hidden environment variables or Cargo configuration are overriding it.
4. Whether the build is native Apple Silicon or accidentally using an x86/Rosetta path.
5. Whether Xcode, Command Line Tools, Clang, SDK selection, and the linker are internally consistent.
6. Whether linker-related Rust profile settings are appropriate for fast development.
7. Whether the resulting configuration is actually optimal for the workload rather than merely theoretically correct.

For a modern Apple-Silicon Mac doing local Rust development, the desired default is generally:

```text
Cargo
  ↓
rustc
  ↓
Darwin compiler-driver linker interface
  ↓
Apple Clang / cc
  ↓
Apple ld
  ↓
ld-prime / new linker
  ↓
Mach-O executable
```

Rust's linker architecture distinguishes the compiler driver from the underlying linker. On Unix-style targets, `rustc` commonly invokes `cc`/Clang as the linker *driver*, which then invokes the platform linker with the required system-library and SDK arguments. Rust's Darwin targets use the Darwin linker flavor, while Cargo only overrides the linker when configuration explicitly tells it to do so. citeturn393635search0turn393635search3turn176371search0

---

# 1. Define the Gold-Standard State

For local Rust development on a current Apple-Silicon Mac, I would consider the configuration healthy when all of the following are true:

| Layer | Desired state |
|---|---|
| CPU execution | Native `arm64`, not Rosetta |
| Rust host | `aarch64-apple-darwin` |
| Rust target | Normally `aarch64-apple-darwin` |
| Xcode/CLT | Current installation |
| Developer directory | Deliberately selected and valid |
| Compiler driver | Apple `cc` / Apple Clang |
| Linker | Apple's current `ld` |
| Linker implementation | ld-prime/new linker |
| Cargo `linker=` | Usually **unset** |
| `-fuse-ld=lld` | Absent unless intentionally benchmarked |
| `ld64.lld` / `rust-lld` | Absent unless intentionally selected |
| `-ld_classic` | Absent |
| `-ld_new` | Usually unnecessary; acceptable if deliberately forced |
| Dev incremental compilation | Enabled |
| Dev debug data | Reduced if build speed matters |
| macOS split debuginfo | Normally Cargo's default `unpacked` |
| Dev LTO | Disabled |
| Linker choice | Validated with the actual project workload |

Apple introduced ld-prime in Xcode 15 and made the new linker the default for macOS and other Apple-platform binaries. `-ld_classic` and `-ld_new` can select implementations in Xcode generations that still ship both. Apple's linker documentation describes these selectors explicitly. citeturn722875search0turn176371search6

---

# 2. Start the Audit From the Actual Project Directory

This matters more than it appears.

Cargo configuration is hierarchical. Cargo searches for `.cargo/config.toml` or `.cargo/config` beginning at the current working directory and proceeding through parent directories, then incorporates the Cargo-home configuration.

Therefore perform the audit from exactly where Cursor or your programming agent normally invokes Cargo:

```bash
cd /path/to/your/workspace
pwd
```

Cargo's hierarchical behavior means that auditing only:

```bash
~/.cargo/config.toml
```

is insufficient. A repository, parent directory, or even higher-level workspace directory can contain another Cargo configuration. citeturn568311search0

---

# 3. Verify That the Mac Is Running Natively

Run:

```bash
uname -m
arch
rustc -vV
```

For an Apple-Silicon machine, the desired results are:

```text
arm64
arm64
...
host: aarch64-apple-darwin
```

Also test whether the shell itself is running through Rosetta:

```bash
sysctl -in sysctl.proc_translated 2>/dev/null || echo "not translated"
```

Interpretation:

```text
0
```

or an unavailable value is normally fine.

```text
1
```

means the current process is being translated under Rosetta.

### Why this matters

An Apple-Silicon machine can still run an x86_64 terminal, IDE helper process, Rust toolchain, or other development component under Rosetta. That creates unnecessary translation overhead and can lead to:

- x86_64 Rust targets
- architecture-specific rebuilds
- different dependency artifacts
- unexpected linker paths
- duplicated Cargo caches
- slower execution

For maximum local-build performance, use:

```text
aarch64 process
+
aarch64 Rust host
+
aarch64 target
```

unless cross-compilation is intentional.

---

# 4. Identify the Active Apple Developer Toolchain

Run:

```bash
xcode-select -p
```

Typical valid outputs are:

```text
/Applications/Xcode.app/Contents/Developer
```

or:

```text
/Library/Developer/CommandLineTools
```

Neither is inherently wrong.

The important question is whether the selected developer directory contains the toolchain you actually intend to use.

Now run:

```bash
xcrun --find clang
xcrun --find ld
xcrun --show-sdk-path
```

Then:

```bash
xcrun clang --version
```

and:

```bash
xcrun ld -v
```

Also useful:

```bash
xcrun ld -version_details
```

### Desired interpretation

You want:

- Apple Clang
- Apple's `ld`
- a current macOS SDK
- all resolved through the same active developer environment

An actual modern Apple linker version may report information resembling:

```text
PROGRAM:ld
PROJECT:ld-...
configured to support archs: ...
will use ld-classic for: ...
```

An Apple Xcode 16.4 example published on Apple's developer forums shows exactly this style of output. For modern arm64 builds, arm64 is not among the architectures routed to the classic implementation. citeturn722875search7

---

# 5. Determine Whether Full Xcode or Command Line Tools Are Selected

If full Xcode is installed:

```bash
xcodebuild -version
```

You can also inspect Command Line Tools package metadata:

```bash
pkgutil --pkg-info=com.apple.pkg.CLTools_Executables 2>/dev/null
```

The important configuration issue is not simply "Xcode versus CLT." It is **whether `xcode-select` points where you expect it to point**.

For example, you may have a recent `/Applications/Xcode.app`, while:

```bash
xcode-select -p
```

still reports:

```text
/Library/Developer/CommandLineTools
```

That means command-line builds are not necessarily using the toolchain inside the Xcode application.

If full Xcode is your intended canonical toolchain, select it with:

```bash
sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
```

If you deliberately use standalone Command Line Tools, do not change this merely for cosmetic consistency.

---

# 6. Check for a Per-Process Xcode Override

`xcode-select` is not the only mechanism that can select the developer directory.

Run:

```bash
echo "$DEVELOPER_DIR"
```

If empty:

```text

```

good—`xcode-select` controls selection.

If you see something like:

```text
/Applications/Xcode-16.4.app/Contents/Developer
```

then that environment variable overrides the normal selected developer directory for the process.

This can be intentional, but it should not be accidental.

Check simultaneously:

```bash
env | grep -E '^(DEVELOPER_DIR|SDKROOT|MACOSX_DEPLOYMENT_TARGET)='
```

Pay particular attention to stale paths left in:

```text
~/.zshrc
~/.zprofile
~/.profile
Cursor environment configuration
development-agent startup scripts
```

---

# 7. Check for PATH Shadowing

Run:

```bash
type -a cc
type -a clang
type -a ld
```

and:

```bash
which -a cc clang ld
```

Then inspect:

```bash
cc --version
clang --version
```

For the normal Apple path, `cc` should ultimately be Apple Clang.

A Homebrew or custom LLVM installation is not automatically bad, but if the goal is to use Apple's native toolchain, you should know if something such as:

```text
/opt/homebrew/opt/llvm/bin/clang
```

is taking precedence over the Apple compiler.

Do not infer the Rust linker simply from:

```bash
which ld
```

however.

Rust frequently invokes a **compiler driver** such as `cc`, not `ld` directly. The driver subsequently invokes the underlying linker. Rust's documentation explicitly distinguishes these two roles. citeturn393635search0

---

# 8. Audit Environment Variables That Can Override Rust/Cargo

Run:

```bash
env | grep -E \
'^(RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|CARGO_BUILD_RUSTFLAGS|RUSTDOCFLAGS|CARGO_TARGET_.*_LINKER|CARGO_TARGET_.*_RUSTFLAGS|CC|CXX|LD|AR|LDFLAGS|CFLAGS|CXXFLAGS|DEVELOPER_DIR|SDKROOT|MACOSX_DEPLOYMENT_TARGET|CARGO_INCREMENTAL|CARGO_BUILD_INCREMENTAL)='
```

The most important Rust/Cargo variables are:

```text
RUSTFLAGS
CARGO_ENCODED_RUSTFLAGS
CARGO_BUILD_RUSTFLAGS
CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER
CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS
```

Cargo formally supports target-specific linker environment variables of the form:

```text
CARGO_TARGET_<TRIPLE>_LINKER
```

and target-specific Rust flags. citeturn176371search5

### Red flags

Look for:

```text
-fuse-ld=lld
-fuse-ld=mold
ld64.lld
rust-lld
zld
sold
-ld_classic
```

Also inspect any explicit linker path.

### Important precedence rule

Cargo does **not** simply merge every source of Rust flags.

For Rust flags, it checks sources in precedence order, beginning with:

1. `CARGO_ENCODED_RUSTFLAGS`
2. `RUSTFLAGS`
3. matching target-specific Cargo configuration
4. `[build].rustflags`

Thus an environment variable can silently override configuration that looks correct on disk. citeturn176371search0

---

# 9. Audit Every Cargo Configuration File That Can Affect the Project

Start with the project:

```bash
find . -path '*/.cargo/config.toml' -o -path '*/.cargo/config'
```

Then inspect global Cargo configuration:

```bash
ls -la ~/.cargo/config ~/.cargo/config.toml 2>/dev/null
```

Search likely configuration locations:

```bash
grep -RniE \
'linker|rustflags|link-arg|link-args|fuse-ld|lld|mold|zld|sold|ld_classic|ld_new' \
.cargo ~/.cargo/config ~/.cargo/config.toml 2>/dev/null
```

Remember that Cargo searches **parent directories as well**.

A more robust audit is:

```bash
d="$PWD"

while :; do
    for f in "$d/.cargo/config" "$d/.cargo/config.toml"; do
        if [ -f "$f" ]; then
            echo
            echo "===== $f ====="
            grep -nEi \
              'linker|rustflags|link-arg|link-args|fuse-ld|lld|mold|zld|sold|ld_classic|ld_new' \
              "$f" || true
        fi
    done

    [ "$d" = "/" ] && break
    d="$(dirname "$d")"
done

for f in \
    "${CARGO_HOME:-$HOME/.cargo}/config" \
    "${CARGO_HOME:-$HOME/.cargo}/config.toml"
do
    if [ -f "$f" ]; then
        echo
        echo "===== $f ====="
        grep -nEi \
          'linker|rustflags|link-arg|link-args|fuse-ld|lld|mold|zld|sold|ld_classic|ld_new' \
          "$f" || true
    fi
done
```

Cargo's documented hierarchy makes this parent-directory search important. citeturn568311search0

---

# 10. Know What a Clean Cargo Configuration Looks Like

For the Apple linker specifically, the ideal configuration frequently looks like:

```toml
# Nothing.
```

In other words, you generally do **not** need:

```toml
[target.aarch64-apple-darwin]
linker = "..."
```

and you generally do not need:

```toml
rustflags = [
    "-C",
    "link-arg=-fuse-ld=..."
]
```

Cargo's documented default for `target.<triple>.linker` is **no user override**. Rust then infers the linker appropriate to the target. citeturn176371search0turn393635search0

This is desirable because it allows the Apple toolchain itself to select the correct current linker implementation and SDK.

---

# 11. Ask rustc What Linker It Actually Invokes

This is one of the most valuable tests in the entire audit.

Modern `rustc` exposes:

```text
--print link-args
```

which performs the link and prints the **full linker invocation**, including the linker executable and arguments. This is specifically documented as a linker-debugging facility. citeturn336219search0

For a simple executable crate:

```bash
cargo rustc -- --print link-args
```

For a workspace binary:

```bash
cargo rustc -p PACKAGE --bin BINARY -- --print link-args
```

For a specific integration-test target:

```bash
cargo rustc -p PACKAGE --test TEST_NAME -- --print link-args
```

You may see something beginning with:

```text
"cc" ...
```

or:

```text
"clang" ...
```

This is normal and desirable.

### What to search for

Capture it:

```bash
cargo rustc -p PACKAGE --bin BINARY -- --print link-args \
    2>&1 | tee /tmp/rust-link-args.txt
```

Then:

```bash
grep -Ei \
'(^|[ ="])(cc|clang|ld)|fuse-ld|lld|mold|zld|sold|ld_classic|ld_new|-B' \
/tmp/rust-link-args.txt
```

### Interpretation

#### Good

```text
cc ...
```

or:

```text
clang ...
```

where that command resolves to Apple Clang, with:

```text
no -fuse-ld=lld
no ld64.lld
no -ld_classic
```

On Xcode 15 or later, Apple's new linker is the platform default. citeturn722875search0

#### Explicitly new Apple linker

Something containing:

```text
-Wl,-ld_new
```

This explicitly selects the new implementation.

It is acceptable, but normally redundant.

#### Old Apple linker

```text
-Wl,-ld_classic
```

This is a performance red flag for ordinary modern development unless you deliberately added it to work around a compatibility problem.

#### LLD

Examples:

```text
-fuse-ld=lld
ld64.lld
rust-lld
```

This means you have intentionally or accidentally selected LLVM's linker instead of Apple's linker.

#### Unexpected direct linker

If the invocation begins with a custom absolute `ld`, LLD, wrapper, or other executable, investigate the Cargo/environment configuration that inserted it.

---

# 12. Verify What `cc` Actually Is

If Rust reports:

```text
cc
```

run:

```bash
command -v cc
cc --version
```

Desired result:

```text
Apple clang version ...
```

If it resolves to something unexpected, inspect:

```bash
type -a cc
```

and your `PATH`:

```bash
printf '%s\n' "${PATH//:/$'\n'}"
```

The combination:

```text
rustc → cc → Apple Clang → Apple ld
```

is a normal and correct Darwin linker chain.

---

# 13. Verify the Underlying Linker Selected by Clang

Create a trivial C program:

```bash
cat >/tmp/linker-audit.c <<'EOF'
int main(void) { return 0; }
EOF
```

Ask the compiler driver to expose its subprocesses:

```bash
cc -### /tmp/linker-audit.c -o /tmp/linker-audit 2>&1
```

Inspect the output for the actual link step.

If `cc` is Apple Clang and no alternate linker selector is present, it should use Apple's Darwin linking environment.

You can separately compare with the explicitly selected Apple developer toolchain:

```bash
xcrun clang -### /tmp/linker-audit.c -o /tmp/linker-audit-xcrun 2>&1
```

If these differ materially, your shell's `cc` and your selected Xcode/CLT environment are not aligned.

---

# 14. Verify That ld-prime Is Available and Is the Default

Run:

```bash
xcrun ld -v
```

On Xcode 15 and later, the new linker is the default for supported modern targets. Apple explicitly documented this when Xcode 15 shipped. citeturn722875search0

You can also inspect whether the installed linker recognizes the implementation selectors:

```bash
xcrun ld -help 2>&1 | grep -E 'ld_classic|ld_new'
```

On toolchains shipping both implementations, Apple documents:

```text
-ld_classic
-ld_new
```

where `-ld_classic` forces the older linker and `-ld_new` forces the newer implementation. citeturn176371search6

### Future-proofing note

Apple has been progressively retiring the classic implementation. Apple's developer documentation notes that Xcode 27 beta removed the older ld64 implementation entirely.

Therefore **do not build a permanent Rust configuration around `-ld_new` unless you actually need it**. Letting the current Apple toolchain use its default is more future-proof. citeturn991072search1

---

# 15. Check for Accidental LLD Selection

Search everything:

```bash
grep -RniE \
'fuse-ld|ld64\.lld|rust-lld|lld' \
.cargo ~/.cargo 2>/dev/null
```

Also:

```bash
env | grep -i lld
```

and inspect your printed Rust link arguments.

LLD is a serious, high-quality linker and may still be appropriate for reproducibility, cross-platform tooling, ThinLTO workflows, or project-specific reasons.

But it should not be present accidentally.

Current Chromium documentation is particularly informative here: Chromium continues to default to LLD for toolchain uniformity and production semantics, but supports Apple ld-prime for local native arm64 development because its recent measurements found Apple's linker significantly faster—up to roughly 4–6× in some Chromium configurations. citeturn991072search0

The correct conclusion is:

```text
LLD = potentially desirable for specific engineering reasons
Apple ld-prime = particularly compelling for local native Mac iteration speed
```

not that one linker universally dominates every workload.

---

# 16. Check for Accidental Classic-Linker Selection

Run:

```bash
grep -RniE \
'ld_classic|ld64' \
.cargo ~/.cargo Cargo.toml 2>/dev/null
```

and:

```bash
env | grep -Ei 'ld_classic|ld64'
```

Also inspect:

```bash
cargo rustc ... -- --print link-args
```

for:

```text
-Wl,-ld_classic
```

If found, determine why it was added.

The classic linker was often used as a workaround for early ld-prime compatibility issues. Apple itself recommended `-ld_classic` as a temporary workaround for specific Xcode 15 linker bugs. citeturn722875search5

That historical workaround should not automatically survive indefinitely in a modern Rust environment.

---

# 17. Do Not Confuse the Linker With the Dynamic Linker

There are two different concepts:

```text
ld
```

is the **static/build-time linker**.

```text
dyld
```

is the **runtime dynamic loader/linker**.

Your Rust compile-time performance concern is primarily:

```text
ld / ld-prime
```

not:

```text
/usr/lib/dyld
```

Apple explicitly distinguishes the static linker from the runtime dynamic linker in its linker documentation. citeturn991072search1

---

# 18. Verify Native Dependencies Separately

A Rust build may use Apple's linker correctly for the final Rust executable while C/C++ dependencies are built through a different toolchain.

Audit:

```bash
env | grep -E \
'^(CC|CXX|AR|CFLAGS|CXXFLAGS|LDFLAGS)='
```

Common native dependencies may use build scripts and tools such as:

```text
cc
clang
clang++
cmake
ninja
pkg-config
```

Therefore distinguish:

```text
Rust final link configuration
```

from:

```text
native dependency compilation/link configuration
```

Both can affect total build time.

---

# 19. Audit the Development Profile, Not Just the Linker

The fastest linker in the world still has to process whatever object/debug data Rust gives it.

Cargo's current build-performance guidance explicitly recommends reducing debug information for fast development builds. citeturn777796search1

A strong development configuration is:

```toml
[profile.dev]
debug = "line-tables-only"

[profile.dev.package."*"]
debug = false

[profile.debugging]
inherits = "dev"
debug = true
```

This gives:

```text
normal dev:
    minimal line information for your code
    no dependency debuginfo

explicit debugging profile:
    full debugger-oriented information
```

Cargo identifies the benefits as:

- faster code generation
- faster linking
- smaller `target` directories

while preserving an opt-in profile for serious debugging. citeturn777796search1

---

# 20. Keep Incremental Compilation Enabled for Development

Cargo's default `dev` profile includes:

```toml
incremental = true
```

along with high codegen parallelism. citeturn777796search0

Check for overrides:

```bash
env | grep -E 'CARGO_INCREMENTAL|CARGO_BUILD_INCREMENTAL'
```

and:

```bash
grep -Rni 'incremental' Cargo.toml .cargo ~/.cargo 2>/dev/null
```

For normal local iterative development, you generally do **not** want:

```text
CARGO_INCREMENTAL=0
```

unless you have measured a project-specific reason.

---

# 21. Keep Development LTO Off

Cargo's normal development profile uses:

```toml
lto = false
```

This is desirable for rapid compile-link-test cycles. citeturn777796search0

Search:

```bash
grep -RniE \
'(^|[^a-z])lto|linker-plugin-lto' \
Cargo.toml .cargo ~/.cargo 2>/dev/null
```

LTO may be valuable for final runtime performance, but it deliberately performs additional whole-program optimization and is not normally appropriate for your fastest development/testing loop.

---

# 22. Leave macOS Split Debuginfo at the Development-Friendly Default

Cargo's current default on macOS for profiles with debug information is:

```text
split-debuginfo = "unpacked"
``` citeturn777796search0


This means debug information remains associated with compilation objects rather than requiring the normal development build to package a complete dSYM-style debug bundle.

Do not change this merely because another platform recommends something different.

For normal rapid development, I would leave it alone unless benchmarking shows otherwise.

---

# 23. Understand the DWARF / `__eh_frame` 16 MB Warning

The warning you previously described:

```text
__eh_frame section too large (max 16MB)
...
dwarf unwind offsets
...
compact unwind table
```

does **not** demonstrate that the wrong linker is configured.

A large Rust binary can legitimately generate enough unwind information to hit Apple's compact-unwind encoding limitation.

Therefore treat these as separate questions:

```text
Question 1:
Am I using the intended linker?

Question 2:
Does this binary generate an unusually large unwind-information section?
```

Do not switch linkers merely to eliminate the warning unless testing shows a concrete advantage.

---

# 24. Avoid Permanent Linker Flags Unless Necessary

It is tempting to add:

```toml
[target.aarch64-apple-darwin]
rustflags = [
    "-C",
    "link-arg=-Wl,-ld_new",
]
```

just to make the intended linker explicit.

I generally would **not** do this.

Apple's new linker is already the default on Xcode 15+. citeturn722875search0

Keeping the configuration implicit has advantages:

```text
less configuration
less chance of future incompatibility
less chance of stale copied settings
easier toolchain upgrades
clearer Cargo configuration
```

Use explicit `-ld_new` primarily as a diagnostic experiment, not as a default requirement.

---

# 25. Be Careful With Global `RUSTFLAGS`

A global setting such as:

```bash
export RUSTFLAGS="..."
```

affects a huge portion of the Cargo graph.

Changing compiler flags can also cause otherwise reusable artifacts to need rebuilding.

Therefore avoid treating global `RUSTFLAGS` as your routine linker configuration mechanism.

Prefer:

- Cargo profiles for profile behavior
- target configuration when an actual target-specific override is necessary
- temporary shell variables for experiments

Cargo itself warns that low-level compiler flags can conflict with Cargo-managed behavior and future compiler changes. citeturn176371search0

---

# 26. Validate the Built Architecture

After building:

```bash
cargo build
```

inspect the executable:

```bash
file target/debug/YOUR_BINARY
```

Desired Apple-Silicon output should indicate:

```text
Mach-O 64-bit executable arm64
```

If you unexpectedly see:

```text
x86_64
```

investigate:

```text
Rust host
explicit --target
build.target
Rosetta
environment variables
Cargo configuration
```

Check the target configuration:

```bash
grep -Rni 'target =' Cargo.toml .cargo ~/.cargo 2>/dev/null
```

and:

```bash
echo "$CARGO_BUILD_TARGET"
```

---

# 27. Inspect the macOS Deployment Target

Run:

```bash
rustc --print deployment-target
```

Rust now exposes the selected Apple deployment target directly. citeturn336219search0

Also inspect:

```bash
echo "$MACOSX_DEPLOYMENT_TARGET"
```

An unnecessarily old or unexpectedly overridden deployment target is not primarily a linker-speed problem, but it is part of validating that the whole Apple toolchain is internally intentional.

---

# 28. Benchmark the Workflow That Actually Matters

Do not declare a linker "optimal" based solely on a microbenchmark.

For an LLM programming agent, the key workflow is commonly:

```text
edit
↓
incremental compile
↓
link test executable(s)
↓
run tests
↓
repeat
```

The most useful benchmark is therefore the actual incremental test cycle.

First build:

```bash
cargo test --no-run
```

Then make a representative small source edit—or simply touch a workspace source file for a crude test—and measure:

```bash
/usr/bin/time -p cargo test --no-run
```

Repeat several times.

`--no-run` is useful because it isolates:

```text
Cargo + rustc + codegen + linking
```

from most actual test execution.

Then separately measure:

```bash
/usr/bin/time -p cargo test
```

This allows you to distinguish build/link latency from runtime/test/Gatekeeper latency.

---

# 29. Use Cargo Timing Information

Run:

```bash
cargo build --timings
```

or:

```bash
cargo test --no-run --timings
```

Cargo will generate build timing information useful for determining whether your bottleneck is actually linking versus:

```text
dependency compilation
procedural macros
build scripts
code generation
serialization
native libraries
```

A linker change cannot meaningfully fix a workflow dominated by another phase.

---

# 30. Benchmark With Warm Caches

For an LLM coding agent, **warm incremental performance matters more than clean-build performance**.

Measure both:

### Clean build

```bash
cargo clean
/usr/bin/time -p cargo test --no-run
```

Use this sparingly because it destroys useful cache state.

### Warm build

```bash
cargo test --no-run
touch path/to/a/relevant/source.rs
/usr/bin/time -p cargo test --no-run
```

Repeat the warm measurement several times.

The second result better approximates an agent repeatedly modifying code and testing it.

---

# 31. Do Not Mix Benchmark Configurations in the Same Cache Blindly

Changing:

```text
RUSTFLAGS
linker
debug information
target CPU
features
toolchain
```

can result in distinct or invalidated build artifacts.

If conducting serious A/B testing, consider separate target directories:

```bash
CARGO_TARGET_DIR=target-apple ...
```

versus:

```bash
CARGO_TARGET_DIR=target-alternative ...
```

This avoids misleading results caused by one configuration benefiting from artifacts produced by another.

---

# 32. Recommended Optimization Priority

For a large Rust workspace, I would optimize in this order:

```text
1. Native arm64 execution
2. Current Xcode / Command Line Tools
3. Correct xcode-select / DEVELOPER_DIR
4. Remove stale linker overrides
5. Verify Apple Clang → Apple ld-prime
6. Preserve incremental compilation
7. Reduce development debuginfo
8. Avoid development LTO
9. Eliminate unnecessary dependencies/features
10. Benchmark real incremental test cycles
11. Only then compare alternative linkers
```

This prevents "linker optimization" from becoming a distraction from larger build-performance problems.

---

# 33. Recommended Cargo Development Profile

For a large workspace where build/test iteration is the priority:

```toml
[profile.dev]
debug = "line-tables-only"

[profile.dev.package."*"]
debug = false

[profile.debugging]
inherits = "dev"
debug = true
```

I would initially leave these unspecified because Cargo's development defaults are already appropriate:

```toml
# incremental = true
# lto = false
# split-debuginfo = "unpacked"   # macOS default
```

Only override defaults when there is a measured reason.

This debug-information recommendation is directly aligned with Cargo's current official build-performance guide. citeturn777796search1

---

# 34. Configuration I Would Remove Unless Deliberately Required

I would investigate and normally remove old snippets such as:

```toml
[target.aarch64-apple-darwin]
linker = "clang"
```

if they serve no documented purpose.

Likewise:

```toml
rustflags = [
    "-C",
    "link-arg=-fuse-ld=lld"
]
```

or:

```toml
rustflags = [
    "-C",
    "link-arg=-Wl,-ld_classic"
]
```

or configurations referencing:

```text
zld
sold
mold
rust-lld
ld64.lld
```

Again, LLD is not inherently wrong. The issue is **unintentional legacy configuration**.

---

# 35. One-Shot System Audit

The following command block provides a useful first snapshot:

```bash
echo '===== SYSTEM ====='
sw_vers
echo
uname -m
arch
printf 'Rosetta translated: '
sysctl -in sysctl.proc_translated 2>/dev/null || echo 'no'

echo
echo '===== RUST ====='
rustc -vV
echo
rustup show active-toolchain 2>/dev/null || true
echo
printf 'Rust deployment target: '
rustc --print deployment-target 2>/dev/null || true

echo
echo '===== APPLE DEVELOPER TOOLS ====='
printf 'Developer dir: '
xcode-select -p
echo
printf 'Clang: '
xcrun --find clang
printf 'ld: '
xcrun --find ld
printf 'SDK: '
xcrun --show-sdk-path
echo
xcrun clang --version
echo
xcrun ld -v 2>&1 | head -20

echo
echo '===== SHELL TOOL RESOLUTION ====='
type -a cc 2>/dev/null || true
type -a clang 2>/dev/null || true
type -a ld 2>/dev/null || true

echo
echo '===== RELEVANT ENVIRONMENT ====='
env | grep -E \
'^(RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|CARGO_BUILD_RUSTFLAGS|RUSTDOCFLAGS|CARGO_TARGET_.*_LINKER|CARGO_TARGET_.*_RUSTFLAGS|CC|CXX|LD|AR|LDFLAGS|CFLAGS|CXXFLAGS|DEVELOPER_DIR|SDKROOT|MACOSX_DEPLOYMENT_TARGET|CARGO_INCREMENTAL|CARGO_BUILD_INCREMENTAL)=' \
|| echo 'No relevant overrides found'
```

Run this from the same terminal/environment in which Cursor launches your Rust work if possible.

---

# 36. Project-Level Configuration Audit

From the workspace root:

```bash
echo '===== PROJECT CARGO/LINKER CONFIGURATION ====='

d="$PWD"

while :; do
    for f in "$d/.cargo/config" "$d/.cargo/config.toml"; do
        if [ -f "$f" ]; then
            echo
            echo "===== $f ====="
            cat "$f"
        fi
    done

    [ "$d" = "/" ] && break
    d="$(dirname "$d")"
done

for f in \
    "${CARGO_HOME:-$HOME/.cargo}/config" \
    "${CARGO_HOME:-$HOME/.cargo}/config.toml"
do
    if [ -f "$f" ]; then
        echo
        echo "===== $f ====="
        cat "$f"
    fi
done
```

Then search specifically for suspicious settings:

```bash
grep -RniE \
'linker|rustflags|link-arg|link-args|fuse-ld|lld|mold|zld|sold|ld_classic|ld_new|incremental|split-debuginfo|lto' \
.cargo Cargo.toml ~/.cargo/config ~/.cargo/config.toml 2>/dev/null
```

---

# 37. Decision Tree

Use the following interpretation:

```text
Is machine/process arm64?
│
├─ No → resolve Rosetta/x86 toolchain first
│
└─ Yes
    │
    ├─ Is Rust host aarch64-apple-darwin?
    │   ├─ No → fix Rust toolchain/target
    │   └─ Yes
    │
    ├─ Is xcode-select pointing to intended tools?
    │   ├─ No → fix developer-directory selection
    │   └─ Yes
    │
    ├─ Is xcrun clang Apple Clang?
    │   ├─ No → developer-tool installation problem
    │   └─ Yes
    │
    ├─ Is xcrun ld modern Apple ld?
    │   ├─ No → update/fix Xcode or CLT
    │   └─ Yes
    │
    ├─ Cargo/environment linker override present?
    │   ├─ Yes → determine whether intentional
    │   └─ No
    │
    ├─ rustc --print link-args reaches Apple cc/clang?
    │   ├─ No → find override
    │   └─ Yes
    │
    ├─ -fuse-ld=lld / ld64.lld?
    │   ├─ Yes → LLD is selected
    │   └─ No
    │
    ├─ -ld_classic?
    │   ├─ Yes → classic Apple linker selected
    │   └─ No
    │
    └─ Current Apple toolchain
        → new Apple linker / ld-prime is expected
```

---

# 38. Final Gold-Standard Configuration

For a normal current Apple-Silicon Rust development machine, the configuration I would aim for is:

```text
macOS
└── native arm64 process
    └── rustup stable aarch64-apple-darwin
        └── Cargo
            ├── no custom macOS linker override
            ├── incremental dev builds
            └── reduced development debuginfo
                ↓
              rustc
                ↓
         Darwin cc/Clang linker driver
                ↓
            Apple Clang
                ↓
        current Apple ld / ld-prime
                ↓
            arm64 Mach-O
```

with the project configuration limited to something like:

```toml
[profile.dev]
debug = "line-tables-only"

[profile.dev.package."*"]
debug = false

[profile.debugging]
inherits = "dev"
debug = true
```

and **no linker-specific Cargo configuration at all**.

---

# 39. Pass/Fail Checklist

A system passes the audit when:

```text
[ ] uname/arch = arm64
[ ] not running under Rosetta
[ ] rustc host = aarch64-apple-darwin
[ ] xcode-select points to intended developer tools
[ ] xcrun clang reports Apple Clang
[ ] xcrun ld reports current Apple ld
[ ] macOS SDK path is current/intended
[ ] DEVELOPER_DIR is empty or intentionally set
[ ] no unexpected SDKROOT
[ ] no unexpected MACOSX_DEPLOYMENT_TARGET
[ ] no CARGO_TARGET_*_LINKER override
[ ] no unexpected RUSTFLAGS
[ ] no -fuse-ld=lld unless intentional
[ ] no ld64.lld/rust-lld unless intentional
[ ] no -ld_classic
[ ] rustc --print link-args uses Apple cc/Clang path
[ ] output binary is arm64 Mach-O
[ ] dev incremental compilation remains enabled
[ ] dev LTO is off
[ ] dev debuginfo has been evaluated for build-speed optimization
[ ] actual warm cargo test --no-run performance has been measured
```

If every item passes, I would regard the Apple-linker configuration as both **correctly configured and strongly aligned with current best practice for high-performance local Rust development on macOS**.

The final determination of "optimal," however, should always come from timing your actual large Rust workspace rather than from linker reputation alone. Cargo's own build-performance guidance makes the same recommendation: measure configuration changes against the workflows you actually care about because build-performance optimizations can be workload-dependent. citeturn777796search1