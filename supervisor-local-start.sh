

#!/bin/bash
# This script builds the project, creates the necessary folder structure
# and starts the supervisor.

# Meant to be used inside a dev container

cargo build

mkdir -p build/instance/configs && mkdir -p build/instance/modules && mkdir -p build/instance/modules
cp -r instance/configs build/instance/configs
cp -r instance/modules build/instance/modules
cp -r instance/outputs build/instance/outputs
cp .env build/.env
set -e

if [ -f build/.env ]; then
  export $(grep -v '^#' build/.env | sed 's/\s*#.*//' | xargs)
else
  echo "❌ Error: .env file not found. Please create one if you run into issues."
fi

# Start dbus
if ! pgrep -x dbus-daemon > /dev/null; then
  echo "🔌 Starting dbus-daemon..."
  rm -f /run/dbus/pid # Remove stale pid file if exists
  dbus-daemon --system --fork
else
  echo "✅ dbus-daemon already running."
fi

# Start avahi
if ! pgrep -x avahi-daemon > /dev/null; then
  echo "🌐 Starting avahi-daemon..."
  avahi-daemon --no-drop-root --daemonize --debug
else
  echo "✅ avahi-daemon already running."
fi

# Copy the supervisor executable to the build folder
cp target/debug/supervisor build/supervisor

# Start the supervisor
echo "🚀 Starting the supervisor..."
build/supervisor
