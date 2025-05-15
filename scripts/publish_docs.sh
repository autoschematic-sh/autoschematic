#!/bin/bash
set -eo pipefail
META_VERSION=$(cat VERSION)
pushd docs.autoschematic.sh
mkdocs build
rsync -avz -e 'ssh -p 909' --chown=1000:1000 --chmod=Du=rwx,Dg=rx,Do=rx,Fu=rw,Fg=r,Fo=r ./site/ $DOCS_SSH_URL:/home/share/www/docs.autoschematic.sh/
popd