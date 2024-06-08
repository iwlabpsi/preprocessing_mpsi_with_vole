English version: [README.md](./README.md)

# The Rust project for Multi-party Private Set Intersection with Preprocessing

詳細は論文 ["前処理型多者間秘匿積集合計算プロトコル"](https://iw-lab.jp/research/scis-oshiw24/) を読んでください。

本プロジェクトはUbuntu等のLinux系OS上でのみ実行可能です。

## Getting Start

[Rust](https://www.rust-lang.org/learn/get-started) をインストールし、本プロジェクトを[クローン](https://docs.github.com/en/repositories/creating-and-managing-repositories/cloning-a-repository?platform=linux)してください。

クローンしてきたディレクトリ上に移り、以下を実行してください。

```bash
cargo run --bin=prep_psi
```

Ubuntu OS上でインストールする場合は、[for_ubuntu_jp.md](for_ubuntu_jp.md) に詳細が書いています。

## オプション

`cargo run --bin=prep_psi -- --help` というコマンドでもオプション一覧を確認できます。

| Name            | Alias | Default | Description                                                                                                            |
| :-------------- | :---: | :-----: | :--------------------------------------------------------------------------------------------------------------------- |
| `--num_parties` | `-N`  |    3    | プロトコル参加パーティ数                                                                                               |
| `--set_size`    | `-n`  |   10    | 各参加者が持つ集合サイズ                                                                                               |
| `--common_size` | `-m`  |    5    | 全参加者の持つ集合の共通集合のサイズ                                                                                   |
| `--vole`        | `-v`  |  `lpn`  | VOLE共有方法                                                                                                           |
|                 |       |         | Possible Value: `lpn` (Learning Parity with Noise assumption) または `ot` (Oblivious Transfer)                         |
| `--solver`      | `-s`  | `paxos` | 使用するソルバ                                                                                                         |
|                 |       |         | Possible Value: `vandelmonde` または `paxos` (PaXoS: Probe-and-XOR of Strings)                                         |
| `--channel`     | `-c`  | `unix`  | 使用するチャネル形式                                                                                                   |
|                 |       |         | Possible Value: `unix` (Unix domain socket), `tcp`, `cross-beam` (Rustが持つネイティブのチャネル)                      |
| `--port`        | `-p`  |  10000  | TCPチャネルを使用する場合のポート番号 (内部的に使用するものです。外部と通信する機能は実装していません。申し訳ないです) |
| `--threads`     | `-t`  |  `on`   | マルチスレッド最適化を行うか                                                                                           |
|                 |       |         | Possible Value: `on` or `off`. オフはシングルスレッドを意味しません。パーティ数分のスレッドは作成されます。            |
| `--verbose `    |       |         | 饒舌モード。指定された場合、集合及び共通集合が表示されます。                                                           |

## ベンチマーク

本プロジェクトのベンチマークは [criterion](https://docs.rs/criterion/latest/criterion/) ライブラリを使用して取りました。

測定には以下のようなコマンドを打ってください。

```bash
cargo bench preprocessed_svole_poly_time
```

`cargo bench [name]` の `[name]` は `benchmark_group` である必要があります。

詳細は [criterion documentation](https://bheisler.github.io/criterion.rs/book/index.html) を読んでください。

## ドキュメント

ここに置いておきます: [preprocessing_mpsi_with_vole]()

または以下のコマンドを打つことで手元にドキュメントを作成できます。

```bash
cargo doc
RUSTDOCFLAGS="--html-in-header katex.html" cargo doc --no-deps
```

## 実行可能プログラム (エントリーポイント)

CLI実行可能プログラムは次の場所に置いてあります。

| Binary Name | File Path                                                     |
| :---------- | :------------------------------------------------------------ |
| prep_psi    | [src/preprocessed/psi/main.rs](/src/preprocessed/psi/main.rs) |
| kmprt       | [src/kmprt17/main.rs](/src/kmprt17/main.rs)                   |

## さらなる詳細について

ソースコード中に書かれているドキュメント(コメント)やテストコードを参考にしてください。

- Rustのテストについて: <https://doc.rust-jp.rs/book-ja/ch11-01-writing-tests.html>

## 参考文献等

FYI.

- [swanky library](https://github.com/GaloisInc/swanky)
- [前処理型多者間秘匿積集合プロトコル](https://iw-lab.jp/research/scis-oshiw24/)
- [PSI from PaXoS: Fast, Malicious Private Set Intersection](https://eprint.iacr.org/2020/193)
- [VOLE-PSI: Fast OPRF and Circuit-PSI from Vector-OLE](https://eprint.iacr.org/2021/266)
