#!/bin/sh
if [ "$#" -eq 1 ] && [ "$1" = "--version" ]; then
    printf 'uv 0.0.0 (M11A negative-test fixture)\n'
    exit 0
fi
printf 'The wrong-version fixture must never execute a resolver command.\n' >&2
exit 99
