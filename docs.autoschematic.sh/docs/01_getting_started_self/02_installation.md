# Installation

If all you need is the Autoschematic CLI (for sealing secrets and config tasks), 
the quickest way is using cargo:
```
cargo install autoschematic
```

Note that all packages update in lockstep against the server version. Therefore, if you are
on server version 0.5.0, you should run:
```
cargo install autoschematic@0.5.0
```

## Using Docker

Installing from Docker is as simple as:
```
docker pull autoschematicsh/autoschematic
```

Note that more configuration is necessary to get up and running. 

## Using Cargo

```
# Also installs autoschematic-cli
cargo install autoschematic
```


## Building from source

For the latest changes, you can clone the repo directly:

```
git clone https://github.com/autoschematic-sh/autoschematic
cd autoschematic
cargo build --release
```