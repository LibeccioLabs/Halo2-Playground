FROM rust:1.75

WORKDIR /usr/src/halo2-playground
COPY . .
RUN cargo test --release --no-run

ENTRYPOINT ["cargo", "test", "--release"]
