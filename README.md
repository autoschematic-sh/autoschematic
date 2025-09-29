# Autoschematic

[![MSRV](https://img.shields.io/badge/MSRV-1.90.0-orange)](https://github.com/autoschematic-sh/autoschematic)
[![Crates.io](https://img.shields.io/crates/v/autoschematic-core.svg)](https://crates.io/crates/autoschematic-core)
[![Docs](https://docs.rs/autoschematic-core/badge.svg)](https://docs.rs/autoschematic-core)

[![CI-MacOS](https://github.com/autoschematic-sh/autoschematic/actions/workflows/verify-macos.yml/badge.svg)](https://github.com/autoschematic-sh/autoschematic/actions/workflows/verify-macos.yml)

[![CI-Linux](https://github.com/autoschematic-sh/autoschematic/actions/workflows/verify-ubuntu.yml/badge.svg)](https://github.com/autoschematic-sh/autoschematic/actions/workflows/verify-ubuntu.yml)


Autoschematic is a new framework for Infrastructure-as-Code, written in Rust.

Autoschematic was developed to address the difficulties we experienced while
using the current leading tools, such as Terraform and Pulumi, at small and medium sized companies.

The most urgent among these was _state drift._ State drift was so painful for some teams that it made infrastructure-as-code unworkable; 
state drift even caused serious incidents!

Autoschematic is the result of a research project that targeted users like these: users for whom infrastructure was a cost center, 
who had little time or budget to replatform, and who found themselves unable to rely on their existing infrastructure-as-code as a source of truth.

The results have been extremely promising. With a push-pull state model, Autoschematic can automatically resolve state drift in either direction,
and automatically import your existing manually-created infrastructure into an IaC codebase. (In other words, something like [Terraformer](https://github.com/GoogleCloudPlatform/terraformer) is built-in by design!)

Autoschematic is still in beta; it shouldn't yet be relied on as "Rust's answer to Terraform". It only features a limited number of "connectors" (our analogue to Terraform's "providers"). However, there are a few cool design aspects that distinguish it strongly from current frameworks. 

### A Push-Pull State Model
To mitigate state-drift, Autoschematic is designed around a state model that's **bidirectional**. In other words, you can push _and_ pull, just like git.
The [vscode extension](https://marketplace.visualstudio.com/items?itemName=Autoschematic.autoschematic) is designed to make this more intuitive, as well as providing language server support. This model is also what allows you to scan and import existing resources into IaC automatically.

### Connectors That Speak Their Own Language
You could say Terraform is built around HCL; what language is Autoschematic built around?
The answer is actually that Autoschematic leaves Connectors to handle their own language implementation. This might seem strange, but in practice, it means that the Snowflake connector can speak regular Snowflake DDL SQL, the Kubernetes connector can speak coherent YAML manifests, and the remotefs connector can simply sync raw files as they appear in git, all as native resources of the connector implementations, with language server and state resolution support.

### A Deeper Integration With Github
Autoschematic is actually two implementations, with a shared library (`autoschematic-core`) for common code.
`autoschematic` is the command line program, for local use, and `autoschematic-server` forms a rich Github integration. 
Unlike `atlantis`, Autoschematic works with deleted files in the PR, supports richer formatting and RBAC functionality, and can horizontally scale.

(Note: the docs on hosting this server implementation are not yet complete. )

### So, What Connectors Are There?
Check the [Connector Catalogue](https://autoschematic.sh/catalogue) for the full list.
Feel free to raise an issue here if you'd like to request a connector implementation for a particular service.

# Installation

Note: you will need the protobuf compiler in order to build with `cargo install` as below.

On Mac:
`brew install protobuf`

On Debian/Ubuntu:
`apt-get install protobuf-compiler`

On Red Hat/Fedora:
`dnf install protobuf-compiler`

```shell 
cargo install autoschematic
```

Windows is not natively supported yet; you can use WSL as a workaround.

Now you're about ready to try out some examples!

[https://autoschematic.sh/guide/getting-started/](https://autoschematic.sh/guide/getting-started/)