# syntax=docker/dockerfile:1.3-labs

# The above line is so we can use can use heredocs in Dockerfiles. No more && and \!
# https://www.docker.com/blog/introduction-to-heredocs-in-dockerfiles/

FROM rust:1.90-trixie AS build

# Capture dependencies
RUN cargo new /app
COPY autoschematic-server/Cargo.toml autoschematic-server/Cargo.lock /app/
COPY autoschematic-core/ /autoschematic-core/
COPY autoschematic-macros/ /autoschematic-macros/
# COPY autoschematic/ /autoschematic/

# # We do the same for our app
# RUN cargo new /app
# COPY Cargo.toml /app

# This step compiles only our dependencies and saves them in a layer. This is the most impactful time savings
# Note the use of --mount=type=cache. On subsequent runs, we'll have the crates already downloaded
WORKDIR /app

RUN apt-get update && apt-get install -y \
    protobuf-compiler 
    # python3.11 \
    # libpython3.11-dev 

RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/app/target,id=${TARGETPLATFORM} cargo build --release

# Copy our sources
COPY autoschematic-server/ /app/

# A bit of magic here!
# * We're mounting that cache again to use during the build, otherwise it's not present and we'll have to download those again - bad!
# * EOF syntax is neat but not without its drawbacks. We need to `set -e`, otherwise a failing command is going to continue on
# * Rust here is a bit fiddly, so we'll touch the files (even though we copied over them) to force a new build
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/app/target,id=${TARGETPLATFORM} <<EOF
  set -e
  # update timestamps to force a new build
  touch /app/src/main.rs
  cargo build --release
  cp /app/target/release/autoschematic-server /app
EOF

# Again, our final image is the same - a slim base and just our app
FROM debian:bookworm-slim AS app
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    # python3.11 \ 
    # python3-pip \ 
    # libpython3.11-dev \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /app/autoschematic-server /app/autoschematic-server
EXPOSE 8080
WORKDIR /app
CMD ["/app/autoschematic-server"]
