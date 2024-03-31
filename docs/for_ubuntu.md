# Install manual for Ubuntu

This document show install commands to run this project on Ubuntu.

## Install Rust

Run following commands to install Rust.

```bash
sudo apt update
sudo apt install curl build-essential
cd ~
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

No custom install option are needed. Select `1) Proceed with standard installation (default - just press enter)`.

## Clone the repository

```bash
sudo apt update
sudo apt install git
git clone https://github.com/iwlabpsi/preprocessing_mpsi_with_vole.git
```

## Run the psi

```bash
cargo run --bin=prep_psi
```

See [README.md](README.md) for more details around options.