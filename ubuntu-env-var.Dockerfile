FROM ubuntu:20.04 as build

ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME="/home/indy-cli"
ENV CARGO_HOME="/home/indy-cli"

ARG uid=1001
ARG user=indy-cli

# Update default packages
RUN apt-get update

# Get Ubuntu packages
RUN apt-get install -y \
    build-essential \
    curl \
    cmake \
    git
# Update new packages
RUN apt-get update

RUN useradd -U -ms /bin/bash -u $uid $user

RUN chown -R $user:root $HOME && \
    chmod -R ug+rw $HOME 

# Get Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.27.1

ENV PATH="/home/indy-cli/bin:${PATH}"
RUN rustup default stable && rustup update

USER $user

WORKDIR /home/indy-cli

RUN git clone https://github.com/hyperledger/indy-cli-rs.git

RUN cd indy-cli-rs && cargo build --release

RUN curl -O https://raw.githubusercontent.com/faisalmoeed/test-genesis/main/pool_transactions_genesis

FROM ubuntu:20.04

ARG uid=1001
ARG user=indy-cli

RUN useradd -U -ms /bin/bash -u $uid $user

RUN chown -R $user:root $HOME && \
    chmod -R ug+rw $HOME 

COPY --from=0 /home/indy-cli/indy-cli-rs/target/release/indy-cli-rs /usr/bin

COPY --from=0 /home/indy-cli/pool_transactions_genesis .

USER $user

CMD ["indy-cli-rs"]