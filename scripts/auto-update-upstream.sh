#!/bin/bash
git clone https://$GH_USER:$GH_TOKEN@github.com/byzantion-xyz/aptos-indexer-processors.git upstream
cd upstream
git remote add upstream "https://github.com/aptos-labs/aptos-indexer-processors.git"
git fetch upstream main

git branch -r
# Merge upstream/main with the current branch
git merge upstream/main --no-edit --commit
cd rust
cargo build --locked --release -p processor

# Check if the build was successful
if [ $? -eq 0 ]; then
    cd ..
    pwd
    git push origin -d automerge || :
    git branch -d automerge || :
    git checkout -b automerge
    git commit -a -m "Auto-merge"
    git push -f origin automerge
    gh repo set-default byzantion-xyz/aptos-indexer-processors
    gh pr create --fill --base main --head automerge || :
else
    echo "Build failed"
    exit 1
fi
