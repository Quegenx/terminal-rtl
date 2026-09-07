# Local changes to vte 0.15.0

Upstream: https://github.com/alacritty/vte (Apache-2.0 OR MIT; both license texts retained).

`src/lib.rs` is modified to bound OSC payload accumulation to 9,216 bytes in
both std and fixed-buffer builds. Oversized/invalid OSC is discarded through
BEL or ESC-backslash; malformed control bytes cannot silently alter a link.
The std Vec capacity is at most the next power of two (16 KiB). The regression
feeds eight MiB in fragments and checks length and capacity after every chunk.

OSC 8 splits only its first two semicolons, preserving complete URI data.
Other OSC parameter overflow rejects the entire sequence. ST dispatch waits
for its complete ESC-backslash pair, including across chunks. The new
`Perform::osc_rejected` callback clears active hyperlink metadata in vt100.

Upstream tests were updated only for these intentional behavior changes.
`Cargo.lock` supports reproducible standalone vendor tests. Run:

```sh
cargo test --manifest-path vendor/vte/Cargo.toml --locked --lib
cargo test --manifest-path vendor/vte/Cargo.toml --locked --lib --no-default-features
```

Version-based dependency advisory checks do not assess these local changes.
