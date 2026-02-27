"""Fallback entry point for the autoschematic CLI.

When installed via a platform wheel, the native binary is placed directly
in the bin/ (or Scripts/) directory and takes precedence.  This module
only runs if Python's console_scripts wrapper is invoked instead.
"""

import os
import sys
import sysconfig


def main():
    scripts_dir = sysconfig.get_path("scripts")
    if scripts_dir is None:
        print("autoschematic: unable to locate scripts directory", file=sys.stderr)
        sys.exit(1)

    binary = os.path.join(scripts_dir, "autoschematic")
    if os.name == "nt":
        binary += ".exe"

    if not os.path.isfile(binary):
        print(
            "autoschematic: native binary not found. "
            "You may need to reinstall: pip install --force-reinstall autoschematic",
            file=sys.stderr,
        )
        sys.exit(1)

    os.execv(binary, [binary] + sys.argv[1:])


