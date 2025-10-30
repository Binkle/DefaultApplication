#!/bin/sh

PORT="${TAURI_DEV_PORT:-1420}"

if command -v lsof >/dev/null 2>&1; then
  PIDS="$(lsof -ti "tcp:${PORT}")"
  if [ -n "$PIDS" ]; then
    echo "Port ${PORT} is busy. Terminating leftover processes: ${PIDS}"
    # shellcheck disable=SC2086
    kill $PIDS >/dev/null 2>&1
    sleep 1
  fi
fi

exit 0
