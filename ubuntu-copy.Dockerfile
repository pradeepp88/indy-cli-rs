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

RUN useradd -U -ms /bin/bash -u $uid $user

RUN chown -R $user:root $HOME && \
    chmod -R ug+rw $HOME

# Get Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.27.1
ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup default stable && rustup update

COPY . .

RUN cargo build --release

RUN pwd 

RUN ls -la

# COPY /home/indy-cli-rs/target/release/indy-cli-rs /usr/bin

USER $user

CMD ["indy-cli-rs"]