# Python `blake3` — advanced technical reference / hashlib-compatible BLAKE3 bindings for LLM coding agents

## Version / source anchors

**Release anchor:** `blake3 == 1.0.9`, released 2026-06-22 and current as of 2026-08-20. Python >= 3.8. Rust-backed PyO3 extension with wheels for major platforms, including CPython 3.14/free-threaded combinations documented on PyPI.

Primary anchors:

- https://pypi.org/project/blake3/1.0.9/
- https://github.com/oconnor663/blake3-py
- https://github.com/BLAKE3-team/BLAKE3
- https://blake3.io/

## CodeFabric role

Hash canonical bytes returned by `rfc8785.dumps` in ordinary unkeyed mode and format the 32-byte default digest as lowercase hex with a `b3:` prefix.

```python
from blake3 import blake3

def fingerprint(canonical: bytes) -> str:
    return "b3:" + blake3(canonical).hexdigest()
```

## Capability inventory

The bindings expose the full BLAKE3 modes through a hashlib-like API:

- one-shot and incremental hashing;
- `digest()` and `hexdigest()`;
- extendable output with caller-selected length;
- XOF seeking with `seek=`;
- 32-byte keyed hashing;
- derive-key context mode;
- configurable multithreading via `max_threads` and `blake3.AUTO`;
- memory-mapped file hashing via `update_mmap`;
- `copy()` to branch incremental state.

## Installation

```bash
uv add 'blake3==1.0.9'
# or
python -m pip install 'blake3==1.0.9'
```

Most environments install a binary wheel. Source builds require a Rust toolchain because the package binds the official Rust implementation.

## One-shot hashing

```python
from blake3 import blake3
raw = blake3(data).digest()       # 32 bytes by default
hex_text = blake3(data).hexdigest()  # 64 lowercase hex chars
```

Input accepts bytes-like objects such as `bytes`, `bytearray`, and `memoryview`.

## Incremental hashing

```python
h = blake3()
h.update(part1)
h.update(part2)
digest = h.digest()
```

`update` returns the hasher, permitting fluent chaining, but explicit statements are often clearer in protocol code.

## Copying state

```python
base = blake3(prefix)
a = base.copy().update(a_suffix).digest()
b = base.copy().update(b_suffix).digest()
```

This is useful for common-prefix hashing. Do not use it if it obscures what bytes define a canonical identity.

## XOF / seekable output

```python
extended = blake3(b"foo").digest(length=100)
segment = blake3(b"foo").digest(length=25, seek=75)
```

The default digest is the first 32 bytes of the XOF. CodeFabric `b3:` fixes length at 32 bytes, so never specify a different length for canonical fingerprints.

## Keyed mode

```python
mac = blake3(message, key=key_32_bytes).digest()
```

Key must be exactly 32 bytes. This is a different cryptographic mode from ordinary hashing and is not used for reproducible CodeFabric content fingerprints.

## Key derivation mode

```python
derived = blake3(
    key_material,
    derive_key_context="globally unique application context",
).digest()
```

Context strings should be hardcoded, globally unique, and application-specific. Key material should not be a low-entropy human password.

## Multithreading

```python
single = blake3(data, max_threads=1).digest()
parallel = blake3(data, max_threads=blake3.AUTO).digest()
```

The project warns that multithreading may be slower for inputs shorter than about 1 MB and should be benchmarked for the platform/workload. Canonical JSON records are commonly much smaller, so default single-thread mode is generally best.

Thread count must never affect output bytes.

## Memory-mapped files

```python
h = blake3(max_threads=blake3.AUTO)
h.update_mmap("/large/file")
digest = h.digest()
```

Useful for large-file/source-tree hashing. It is unnecessary for in-memory RFC 8785 canonical bytes.

## Packaging / CPython considerations

1.0.9 upgraded PyO3 and publishes a wide wheel matrix. The release dropped Python 3.13t wheels while continuing current free-threading support where published; CPython 3.14/3.14t wheels are listed for many platforms. Verify the deployment image receives a wheel; otherwise source installation needs Rust.

For hermetic CI, pin the package and lock artifact hashes using the environment's package manager.

## Digest framing

Keep algorithm bytes and protocol text separate:

```python
digest_bytes = blake3(canonical).digest()
assert len(digest_bytes) == 32
framed = "b3:" + digest_bytes.hex()
```

Strict parsing should require exactly `b3:` plus 64 lowercase ASCII hex chars. Do not silently lowercase external input when checking canonical form.

## Error / misuse boundaries

Ordinary in-memory hashing is effectively infallible for valid bytes-like input. Failures arise from wrong argument types, invalid key length/mode combinations, file/mmap operations, allocation/resource errors, or application framing.

A malformed `b3:` identifier should be reported as an application format error, not as a library hashing error.

## Security semantics

BLAKE3 fingerprints establish byte identity. They do not prove authorization or source authenticity. Keyed mode can authenticate with shared secret state, but that is a different protocol.

Do not use ordinary BLAKE3 as password hashing. Do not truncate CodeFabric digests without a new identifier/profile specification.

## Performance guidance

- one-shot hash for canonical byte strings;
- incremental updates for naturally chunked input;
- `memoryview` can avoid unnecessary copies in some adapters;
- `AUTO` threading only after measurement on large data;
- `update_mmap` for large files, not small registry values;
- avoid converting bytes to hex until storage/UI/protocol text needs it.

## Canonicalization anti-patterns

- hashing the Python object repr;
- hashing `json.dumps` output rather than `rfc8785.dumps` bytes;
- accidentally hashing Unicode text after platform-dependent encoding;
- keyed or derive-key mode for ordinary `b3:`;
- `digest(length=...)` with non-32 output under the same prefix;
- uppercase digest output/acceptance where canonical lowercase is required;
- turning on multithreading for tiny records without benchmarking.

## Agent checklist

- [ ] pin is exactly 1.0.9;
- [ ] input is `rfc8785.dumps` bytes;
- [ ] unkeyed/default 32-byte mode is used;
- [ ] text framing is `b3:` + lowercase hex;
- [ ] XOF/keyed/KDF/mmap/threading features are not accidentally introduced into the fingerprint path;
- [ ] raw digest bytes match Rust fixtures.
## Testing matrix for agent-authored changes

At minimum, exercise:

- empty, scalar, nested-object, nested-array, and mixed-value cases;
- ASCII and non-ASCII object keys, including supplementary-plane code points;
- characters requiring JSON escaping and strings containing combining sequences;
- `0`, `-0.0`, integral floats, exponent forms, subnormals, largest finite doubles, and values adjacent to safe-integer boundaries;
- duplicate JSON member names at multiple nesting levels;
- malformed UTF-8 / malformed JSON where the API accepts raw bytes;
- deterministic repeated execution and cross-process execution;
- Rust↔Python byte equality for canonical bytes and BLAKE3 digest framing;
- intentionally malformed `codefabric-bytes`, `codefabric-int64`, `codefabric-uint64`, ID, and digest values;
- very deep / large inputs according to the caller's resource-limit policy.
## Upgrade and compatibility policy

For CodeFabric canonicalization dependencies, a dependency update is **not** an ordinary implementation-only change. Replay the complete shared positive and negative fixture corpus before accepting any upgrade. Positive fixtures must preserve canonical bytes and `b3:` digests byte-for-byte. Negative fixtures must preserve rejection of duplicate names, unsafe integer tokens, non-finite values, malformed typed-format strings, non-canonical base64, and uppercase IDs/digests. If a serializer upgrade changes canonical bytes for any previously accepted value, introduce a new canonicalization profile/version rather than silently changing `codefabric-jcs-v1`.

Agent rule: do not infer compatibility from SemVer alone when a dependency participates in a byte-level protocol.


---

# Extended capability catalog

## 1) Binding model

The Python package is a high-performance binding to the Rust BLAKE3 implementation exposed through a hashlib-like object. The API offers ordinary, keyed, derivation, incremental, XOF, mmap, and optional multithreaded hashing.

CodeFabric uses the smallest subset:

```python
from blake3 import blake3
hex64 = blake3(canonical_bytes).hexdigest()
```

and frames it as `b3:` in application code.

## 2) Constructor and initial data

```python
h = blake3()
h = blake3(b"initial bytes")
```

Supplying initial data is equivalent to constructing then calling `update` with those bytes.

Mode-selection arguments such as `key=` and `derive_key_context=` change cryptographic semantics. They must not appear in the `b3:` checksum path.

## 3) Incremental `update`

```python
h = blake3()
h.update(part1)
h.update(part2)
digest = h.digest()
```

The binding returns the hasher from `update`, enabling chaining, but explicit sequential calls are often easier to debug.

Do not hash structured fields by concatenation without a framing format. CodeFabric hashes one already-canonical JSON byte string instead.

## 4) `digest` / `hexdigest`

Both support ordinary fixed-length output and BLAKE3's extended output controls.

For the CodeFabric checksum contract:

```python
raw32 = h.digest()       # default 32 bytes
hex64 = h.hexdigest()    # default 64 lowercase hex chars
```

If a call specifies a non-default `length` or `seek`, review it as a different protocol use case.

## 5) XOF and `seek`

BLAKE3 can produce arbitrarily long deterministic output, and the Python API can select an offset in that output stream. This is useful for specialized protocols but not for CodeFabric's 256-bit checksum.

Agent red flag in checksum code:

```python
.digest(length=...)
.hexdigest(length=...)
# or seek=...
```

unless the arguments exactly express an independently specified protocol.

## 6) Copying state

`copy()` clones the current hasher state. This is useful when many messages share a common prefix:

```python
prefix = blake3(b"common-prefix")
a = prefix.copy().update(b"A").digest()
b = prefix.copy().update(b"B").digest()
```

Do not use shared-prefix optimization to replace the canonical JSON byte contract. It is appropriate only when the hashed message framing is itself defined that way.

## 7) Keyed mode

A 32-byte secret key selects keyed hashing:

```python
h = blake3(key=key_bytes)
h.update(message)
```

This is not a plain content checksum. Keep authentication keys out of logs and out of the canonical registry object unless explicitly required by a separate security protocol.

## 8) Key derivation mode

`derive_key_context=` selects BLAKE3's context-separated key-derivation behavior. Use a stable descriptive context string for actual KDF use; do not derive keys by ordinary hashing with a manually prepended label.

Again, this mode does not belong in `b3:` checksum calculation.

## 9) `max_threads`

The binding can hash using multiple threads, including an automatic thread-count mode. Parallelism is useful for sufficiently large inputs; package guidance notes that overhead can make it slower for relatively small inputs.

Canonical registry JSON records are normally too small to justify per-record parallel hashing. Parallelize at the document/task level if throughput requires it and ordering semantics remain deterministic.

## 10) `update_mmap`

`update_mmap(path)` hashes file contents through memory mapping. This is useful for large immutable files and content-addressed file workflows.

It does not eliminate JCS serialization: canonical JSON values must first become canonical bytes. Do not write a temporary JSON file merely to call `update_mmap`.

## 11) Packaging and CPython support

`1.0.9` publishes prebuilt wheels across common platforms/interpreters and uses Rust/PyO3 underneath. Build-from-source environments therefore need a compatible Rust toolchain when a wheel is unavailable.

Pin the Python package exactly in the canonicalization environment and test deployment targets that may fall off the wheel matrix.

## 12) Free-threaded / interpreter considerations

Wheel availability can differ for free-threaded CPython builds and architectures. Treat interpreter/package upgrades as deployment changes even though digest semantics should remain stable.

The shared fixture corpus is the correctness gate; packaging smoke tests are the deployment gate.

## 13) GIL and concurrency planning

The native implementation can do substantial work outside pure Python overhead, but application-level parallelism should be benchmark-driven. For many small canonical messages, a Python thread scheduler or native worker fan-out can cost more than BLAKE3 itself.

Batch at a higher level rather than introducing per-hash threading by default.

## 14) CodeFabric checksum helper

```python
from blake3 import blake3

def b3_frame(canonical_bytes: bytes) -> str:
    return "b3:" + blake3(canonical_bytes).hexdigest()
```

The caller must supply **canonical RFC 8785 bytes**, not source JSON text and not `repr()` / normal `json.dumps()` output.

## 15) Verification helper

```python
import re
from blake3 import blake3

_B3 = re.compile(r"b3:[0-9a-f]{64}\Z")

def verify_b3(canonical_bytes: bytes, expected: str) -> bool:
    if _B3.fullmatch(expected) is None:
        return False
    actual = "b3:" + blake3(canonical_bytes).hexdigest()
    return actual == expected
```

If the checksum is a security authentication mechanism rather than a public content digest, use a threat-model-appropriate comparison/API and do not overload this plain checksum helper.

## 16) Failure boundaries

Hashing in-memory bytes does not normally fail. File/mmap operations can raise OS/path errors. API misuse (wrong key length, incompatible mode arguments, invalid lengths) raises Python exceptions.

Application code should not catch `Exception` and relabel all failures as “checksum mismatch.” Distinguish malformed expected checksum, I/O failure, and actual digest mismatch.

## 17) Performance decision table

| Data | API |
|---|---|
| small canonical bytes | `blake3(data).digest/hexdigest` |
| incremental generated stream | `h = blake3(); h.update(...)` |
| large file | `update_mmap` or chunked update after benchmark |
| very large in-memory data | `max_threads` after benchmark |
| checksum | default 32-byte output |
| protocol requiring expansion | explicit XOF length/seek |

## 18) Cross-language parity

The Rust and Python bindings implement the same BLAKE3 algorithm. Cross-language fixtures should compare **raw 32 bytes first** and then application framing:

```text
Rust Hash.as_bytes() == Python digest()
Rust `b3:` + to_hex == Python `b3:` + hexdigest()
```

This helps localize whether a mismatch came from canonical serialization or hash/framing.

## 19) Security and secret-data rules

- plain BLAKE3 checksums are not authentication;
- keyed mode requires secret key lifecycle management;
- KDF mode requires stable domain-separation context;
- never use `b3:` to imply authenticity;
- do not log keyed hasher constructor arguments;
- do not “salt” a checksum ad hoc and still call it the same profile.

## 20) Testing matrix

```text
[ ] official/known BLAKE3 vectors
[ ] empty bytes
[ ] canonical JSON fixture bytes
[ ] incremental chunks == one-shot
[ ] Rust digest bytes == Python digest bytes
[ ] exactly 64 lowercase hex chars
[ ] wrong prefix/case/length rejected by framing layer
[ ] keyed/derive/XOF modes cannot enter checksum helper
[ ] mmap path tested separately if deployed
```

## 21) Agent execution playbook

```text
Use exact Python pin 1.0.9 for this profile.
Use default unkeyed 32-byte output for `b3:`.
Hash rfc8785.dumps(...) bytes, never ordinary JSON source/output.
Keep prefix/case/length validation in application code.
Benchmark threading/mmap; do not add them reflexively.
Compare raw digest bytes with Rust when diagnosing parity failures.
```
