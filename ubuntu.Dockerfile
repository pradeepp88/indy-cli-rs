FROM ubuntu:20.04

ENV DEBIAN_FRONTEND=noninteractive

ARG uid=1001
ARG user=indy-cli-rs

# Update default packages
RUN apt-get update

# Get Ubuntu packages
RUN apt-get install -y \
    build-essential \
    curl \
    cmake
# Update new packages
RUN apt-get update

# Get Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.27.1
ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup default stable && rustup update


RUN useradd -U -ms /bin/bash -u $uid $user

COPY . /usr/src/indy-cli-rs

WORKDIR /usr/src/indy-cli-rs

RUN cargo build --release

RUN chown -R $user:root $HOME && \
    chmod -R ug+rw $HOME /usr/src/indy-cli-rs 

USER $user