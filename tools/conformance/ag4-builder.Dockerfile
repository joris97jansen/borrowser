FROM rust@sha256:5c38a9e735664861e84ce6918e98ce7f05dbac210230d45381bcf870b2d58083
ENV RUSTUP_TOOLCHAIN=1.92.0-x86_64-unknown-linux-gnu
RUN rustc --version | grep '^rustc 1.92.0 '
WORKDIR /source
