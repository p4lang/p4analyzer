# P4 Language Server
A language server for the P4 language equipped with wasm build and VScode extension. The language server, called P4 analyzer, is built in Rust under `crates` directory and compiled to WebAssembly (wasm) for use as a language server. The wasm and VScode extension is in typescript under `packages` directory.

The language server plans to support:
* Code Completion
* Find References
* Variable Information
* Diagnostics
* Jump to Def
* Hover

## Building
The repository supports building on unix & windows systems.

### Prerequisites
* [Rust](https://www.rust-lang.org/tools/install)
* [Wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
* [NodeJS](https://nodejs.org/en/download/)
* `git clone` the repository

### Building using Nx
[Nx](https://nx.dev/) is used as the build system as the repository contains WebAssembly as well as Rust projects. Once the prerequisites have been installed, run the following commands. This will install the required Node.js modules, build the Rust and TypeScript projects, and finally produce a `.vsix` package that can be installed into Visual Studio Code.
```bash
npm ci
npm run build
npm run package
```
