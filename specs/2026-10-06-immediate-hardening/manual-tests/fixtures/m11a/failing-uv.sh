#!/bin/sh
if [ "$#" -eq 1 ] && [ "$1" = "--version" ]; then
    printf 'uv %s (M11A negative-test fixture)\n' "${M11A_UV_VERSION:?Set M11A_UV_VERSION from the manual-test guide}"
    exit 0
fi
printf 'M11A fixture resolver execution failed deliberately.\n' >&2
exit 42
