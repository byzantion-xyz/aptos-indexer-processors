#!/bin/bash
git checkout main
git remote add upstream "https://github.com/aptos-labs/aptos-indexer-processors.git"
git fetch upstream main:upstream_main
git checkout -b automerge


# Merge upstream/main with the current branch
git merge upstream_main
cd rust
#cargo build --locked --release -p processor

# Check if the build was successful
if [ $? -eq 0 ]; then
    cd ..
    git commit -a -m "Auto-merge"
    git push -f -u origin automerge
    
    gh pr create --title "Autoupdate" --body "The upstream/main was merged and built successfully." --base main --head automerge
else
    echo "Build failed"
    exit 1
fi
