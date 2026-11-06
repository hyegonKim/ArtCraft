# Ghidra SDK engine

This crate contains the verified standalone Rust engine and its headless Ghidra adapter. The parent crate retains its PE analysis commands.

Build and test from this directory:

```sh
cargo build --locked --release
cargo test --locked
```

The headless adapter was verified with Ghidra 12.2 PUBLIC and JDK 25. Use an absolute input path, an entry address, and a private workspace path that this process can create and write to.

```sh
./target/release/ghidra-hopper-rs ghidra-decompile /absolute/input.macho 0x100001000 \
  --ghidra-home /absolute/ghidra_12.2_PUBLIC \
  --java-home /absolute/jdk/Contents/Home \
  --project-root /absolute/private-ghidra-workspace \
  --timeout-seconds 180
```
