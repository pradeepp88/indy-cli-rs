FROM rust:1.27.1

ENV DEBIAN_FRONTEND=noninteractive

# # Update default packages
# RUN apt-get upgrade && apt-get update

# Get Ubuntu packages
# RUN apt-get install -y \
#     cmake
# # Update new packages
# RUN apt-get update

RUN rustup default stable && rustup update

COPY . /usr/src/indy-cli-rs