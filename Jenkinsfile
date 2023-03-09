#!/bin/bash

base=`basename $(pwd)`
CONFIG_OPTS=""

# Make sure we build corosync with systemd so that pcs works
if [ "$base" = "corosync" ]
then
    CONFIG_OPTS+="--enable-systemd"
    CONFIG_OPTS+="--enable-rust-bindings"
fi

# Build knet Rust bindings
if [ "$base" = "kronosnet" ]
then
    CONFIG_OPTS+="--enable-rust-bindings"
fi

# Need gnutls for pacemaker-remote (I am not making this up)
if [ "$base" = "corosync" ]
then
    CONFIG_OPTS+="--with-gnutls"
fi

git clean -dxf
sh autogen.sh
./configure "$@" $CONFIG_OPTS
make
make check
make distcheck
