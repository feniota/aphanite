#!/bin/sh
set -eu

config="./config.toml"
previous_arg=""

for arg in "$@"; do
    if [ "$previous_arg" = "config" ]; then
        config=$arg
        previous_arg=""
        continue
    fi

    case "$arg" in
        --config=*)
            config=${arg#--config=}
            ;;
        --config|-c)
            previous_arg=config
            ;;
        init)
            exec /usr/local/bin/aphanite "$@"
            ;;
    esac
done

if [ ! -e "$config" ]; then
    /usr/local/bin/aphanite --config "$config" init
fi

exec /usr/local/bin/aphanite "$@"
