ARG BASE_IMAGE=mcr.microsoft.com/devcontainers/base:trixie

FROM $BASE_IMAGE

RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install -y --no-install-recommends \
    protobuf-compiler \
    clang-format \
    neovim \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*
