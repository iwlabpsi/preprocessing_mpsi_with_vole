Japanese version: [README_jp.md](./README_jp.md)

# The Rust project for Multi-party Private Set Intersection with Preprocessing

See the paper ["Multi-party Private Set Intersection with Preprocessing"](https://iw-lab.jp/research/scis-oshiw24/) for details.

This project only support Linux such as Ubuntu.

## Getting Start

Install [Rust](https://www.rust-lang.org/learn/get-started) and [clone this repository](https://docs.github.com/en/repositories/creating-and-managing-repositories/cloning-a-repository?platform=linux), then run the following command in the cloned directory.

```bash
cargo run --bin=prep_psi
```

See [for_ubuntu.md](for_ubuntu.md) for more details to install on Ubuntu OS.

## Options

You can also type `cargo run --bin=prep_psi -- --help` to see details of the options.

| Name            | Alias | Default | Description                                                                                                              |
| :-------------- | :---: | :-----: | :----------------------------------------------------------------------------------------------------------------------- |
| `--num_parties` | `-N`  |    3    | Number of participants in the protocol.                                                                                  |
| `--set_size`    | `-n`  |   10    | Number of elements of the set that each participant has.                                                                 |
| `--common_size` | `-m`  |    5    | The size of the aggregate product of the sets that each party has.                                                       |
| `--vole`        | `-v`  |  `lpn`  | VOLE Sharing Methods.                                                                                                    |
|                 |       |         | Possible Value: `lpn` (Learning Parity with Noise assumption) or `ot` (Oblivious Transfer)                               |
| `--solver`      | `-s`  | `paxos` | Solver Methods.                                                                                                          |
|                 |       |         | Possible Value: `vandelmonde` or `paxos` (PaXoS: Probe-and-XOR of Strings)                                               |
| `--channel`     | `-c`  | `unix`  | Channel Types.                                                                                                           |
|                 |       |         | Possible Value: `unix` (Unix domain socket), `tcp`, `cross-beam` (Native channel of Rust)                                |
| `--port`        | `-p`  |  10000  | Port number for TCP channel (The port is used internally. No function to communicate externally is implemented. Sorry. ) |
| `--threads`     | `-t`  |  `on`   | Multi-thread optimization.                                                                                               |
|                 |       |         | Possible Value: `on` or `off`. Off doesn't mean single-threaded and at least as many threads are created as parties      |
| `--verbose `    |       |         | Verbose mode. If specified, print the sets and the intersection.                                                         |

## Benchmark

The benchmark of this project is implemented using [criterion](https://docs.rs/criterion/latest/criterion/) library.

To measure, run a command like the following.

```bash
cargo bench preprocessed_svole_poly_time
```

On the command `cargo bench [name]` , `[name]` should be `benchmark_group` .

Please read the [criterion documentation](https://bheisler.github.io/criterion.rs/book/index.html) for more information.

## Documentation

Here: [preprocessing_mpsi_with_vole](https://iwlabpsi.github.io/preprocessing_mpsi_with_vole/preprocessing_mpsi_with_vole/)

Or, you can build the document of the library of this project by running follow script:

```bash
cargo doc
RUSTDOCFLAGS="--html-in-header katex.html" cargo doc --no-deps
```

## Entry Point

Entry points for CLI is written in following files.

| Binary Name | File Path                                                     |
| :---------- | :------------------------------------------------------------ |
| prep_psi    | [src/preprocessed/psi/main.rs](/src/preprocessed/psi/main.rs) |
| kmprt       | [src/kmprt17/main.rs](/src/kmprt17/main.rs)                   |

## For more info

Please read each documents or test code written in source code for more info :)

- About test of Rust: <https://doc.rust-lang.org/book/ch11-00-testing.html>

## Key references

FYI.

- [swanky library](https://github.com/GaloisInc/swanky)
- [前処理型多者間秘匿積集合プロトコル](https://iw-lab.jp/research/scis-oshiw24/)
    - This paper is written in Japanese.
- [PSI from PaXoS: Fast, Malicious Private Set Intersection](https://eprint.iacr.org/2020/193)
- [VOLE-PSI: Fast OPRF and Circuit-PSI from Vector-OLE](https://eprint.iacr.org/2021/266)
