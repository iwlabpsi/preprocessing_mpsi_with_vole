Japanese version: [for_ubuntu_jp.md](./for_ubuntu_jp.md)

# Install manual for Ubuntu

This document shows install commands to run this project on Ubuntu.

## Install Rust

Run the following commands to install Rust.

```bash
sudo apt update
sudo apt install curl build-essential
cd ~
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

No custom install options are required. Select `1) Proceed with standard installation (default - just press enter)`.

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

See [README.md](README.md) for more information on options.

## Tips: WSL

Windows Subsystem for Linux (WSL) is very useful for Windows users to run this project.

See: <https://learn.microsoft.com/en-us/windows/wsl/install>
