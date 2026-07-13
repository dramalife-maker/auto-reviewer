"""Emit >128KiB on stdout then exit 0 — reproduces pipe-buffer deadlock if parent waits before draining."""
import sys

# Well above typical OS pipe buffer (~64KiB on Windows).
sys.stdout.write("x" * (200 * 1024))
sys.stdout.flush()
sys.exit(0)
