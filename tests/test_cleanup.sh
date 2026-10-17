#!/bin/sh
# Removes the test database files created by test_createdb.sh.
#
# Usage:
#   sh tests/test_cleanup.sh
#
# NOTE: this deletes files directly from the server's allowed path.
# Make sure no client is attached to those databases first.
rm -f /database/ps_default.fdb /database/ps_4k.fdb /database/ps_bad.fdb
echo "cleaned"
