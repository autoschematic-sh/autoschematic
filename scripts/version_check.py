#!/usr/bin/python
# Make sure that the version of the crate, wheel etc matches the meta version under VERSION.
# Specifically: for meta(x.y.z), pkg(x.y.z):
# Ensure that meta(x) == pkg(x), meta(y) == pkg(y), meta(z) <= pkg(z)
# Usage: version_check.py META_VERSION PKG_VERSION

import sys

try:
    meta_str = sys.argv[1]
    pkg_str = sys.argv[2]
except:
    print("Usage: version_check.py META_VERSION PKG_VERSION")
    exit(1)

try:
    # Optionally strip the leading v in vX.Y.Z
    if pkg_str[0] == "v":
       pkg_str = pkg_str[1:]
    meta = meta_str.split(".")
    pkg = pkg_str.split(".")
    assert len(meta) == 3
    assert len(pkg) == 3

    int(meta[0]) 
    int(meta[1]) 
    int(meta[2]) 

    int(pkg[0]) 
    int(pkg[1]) 
    int(pkg[2]) 

    assert meta[0] == pkg[0]
    assert meta[1] == pkg[1]
    assert int(meta[2]) <= int(pkg[2])
except:
    print(f"Version check failed: meta version: {meta_str}, package version: {pkg_str}")
    exit(1)
    
exit(0)