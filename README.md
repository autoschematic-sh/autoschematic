# Autoschematic

[![MSRV](https://img.shields.io/badge/MSRV-1.90.0-orange)](https://github.com/autoschematic-sh/autoschematic)
[![Crates.io](https://img.shields.io/crates/v/autoschematic-core.svg)](https://crates.io/crates/autoschematic-core)
[![Docs](https://docs.rs/autoschematic-core/badge.svg)](https://docs.rs/autoschematic-core)

[![CI-MacOS](https://github.com/autoschematic-sh/autoschematic/actions/workflows/verify-macos.yml/badge.svg)](https://github.com/autoschematic-sh/autoschematic/actions/workflows/verify-macos.yml)

[![CI-Linux](https://github.com/autoschematic-sh/autoschematic/actions/workflows/verify-ubuntu.yml/badge.svg)](https://github.com/autoschematic-sh/autoschematic/actions/workflows/verify-ubuntu.yml)


Autoschematic is a tool for managing infrastructure and policy as code.

Unlike Terraform and Pulumi, Autoschematic is built around a push-pull state model. This means that
it can resolve state drift by "pulling" or "pushing" (applying). This makes it a much better fit 
for certain use-cases. See it in action:

https://github.com/user-attachments/assets/80e68971-9d9c-4a63-b834-175a2acc9733

As you can see, this push-pull state model also allows you to import your existing infra automatically. 

You may have also noticed the language server and vscode integration. This is available via the 
[`Autoschematic` extension here.](https://marketplace.visualstudio.com/items?itemName=Autoschematic.autoschematic)


# Installation

Most users can just do:

```bash
pip install autoschematic
```

Windows is not natively supported yet; you can use WSL as a workaround.

If you prefer to build from source, you can also run 
```bash
cargo install autoschematic
```

> Note: to disable fetching the MOTD (fetched no more than once/day otherwise), set the env var AUTOSCHEMATIC_NO_MOTD=1

Now you're about ready to try out some examples.

[https://autoschematic.sh/guide/getting-started/](https://autoschematic.sh/guide/getting-started/)
