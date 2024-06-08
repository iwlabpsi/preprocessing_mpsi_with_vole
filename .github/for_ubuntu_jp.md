English version: [for_ubuntu.md](./for_ubuntu.md)

# Ubuntuのインストール

このドキュメントではUbuntu OS上で本プロジェクトを実行するためのインストールコマンドを示します。

## Rustのインストール

以下のコマンドを実行し、Rustをインストールします。

```bash
sudo apt update
sudo apt install curl build-essential
cd ~
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

カスタムは必要ありません。`1) Proceed with standard installation (default - just press enter)` を選択しそのままインストールしてください。

## リポジトリのクローン

```bash
sudo apt update
sudo apt install git
git clone https://github.com/iwlabpsi/preprocessing_mpsi_with_vole.git
```

## PSI実行

```bash
cargo run --bin=prep_psi
```

詳細は [README.md](README.md) を見てください。

## WSLが便利

Windowsユーザーならば、Windows上で直接実行するのではなく(Unixドメインソケットを使用しているためコンパイルが通りません。)、Windows Subsystem for Linux (WSL)上で実行してみてください。

詳しくは: <https://learn.microsoft.com/ja-jp/windows/wsl/install>
