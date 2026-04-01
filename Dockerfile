FROM ubuntu:24.04

# Install tools useful for testing shells
RUN apt-get update && apt-get install -y \
    sudo \
    passwd \
    procps \
    util-linux \
    strace \
    gdb \
    && rm -rf /var/lib/apt/lists/*

RUN apt-get update && apt-get install -y util-linux

# Create a test user
RUN useradd -m shelltest && \
    echo "shelltest:shelltest" | chpasswd && \
    usermod -aG sudo shelltest

# Install your shell path into /etc/shells
RUN echo /ebash/ebash >> /etc/shells

# Set it as login shell
RUN chsh -s /ebash/ebash shelltest

WORKDIR /home/shelltest
USER shelltest