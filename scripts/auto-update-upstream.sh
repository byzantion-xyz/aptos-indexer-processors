#!/bin/bash
git remote add upstream "https://github.com/aptos-labs/aptos-indexer-processors.git"
git fetch upstream main
git checkout -b upstream_main upstream/main
git checkout main


# Merge upstream/main with the current branch
git merge upstream_main
cd rust
#cargo build --locked --release -p processor

# Check if the build was successful
if [ $? -eq 0 ]; then
    cd ..
    git checkout -b automerge
    git commit -a -m "Auto-merge"
    git push -f origin automerge
    
    gh pr create --fill --base main --head automerge
else
    echo "Build failed"
    exit 1
fi
