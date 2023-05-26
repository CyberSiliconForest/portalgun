#!/bin/bash

# SPDX-FileCopyrightText: 2020-2022 Alex Grinman <me@alexgr.in>
#
# SPDX-License-Identifier: MIT

docker run -v "cargo-cache:$HOME/.cargo/" -v "$PWD:/volume" --rm -it clux/muslrust:stable cargo build --bin tunnelto_server --release

