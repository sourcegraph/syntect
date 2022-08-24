# Workflow tips for iterating on syntect

## Use synhtml to quickly test files

```
# Option 1
cargo build --release --example synhtml # first time
./target/release/examples/synhtml path/to/file.ext

# Option 2 (cargo adds 50ms+ overhead)
cargo run --release --example synhtml -- path/to/file.ext
```

Combine with [`entr`](https://github.com/eradman/entr) if there is no hang.

## Update generated files

If you update the grammars under `testdata/Packages`,
make sure to run `make assets` in the root of this repo
to regenerate the `packdump` files.

## Viewing stacks for hangs

* Linux: `eu-stack -p <pid>` (`sudo apt install elfutils`).
* macOS: `sample <pid>`.
