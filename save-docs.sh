#!/usr/bin/env bash

# Abort if any command fails.
set -e

cd $(dirname $0)

branch=$(git symbolic-ref HEAD 2>/dev/null)
if [ $branch != "refs/heads/master" ]; then
    echo "Error: cannot save docs, not on master branch."
    exit 1
fi

git st | grep "working directory clean" || {
    echo "Error: cannot save docs, working directory isn't clean."
    exit 1
};

cargo doc

TEMP_LOCATION="/tmp/oxischeme-docs"
TEMP_INDEX_LOCATION="/tmp/oxischeme-index.html"

rm -rf $TEMP_LOCATION
mv target/doc $TEMP_LOCATION

message=$(git log -1 --oneline --no-color)

git checkout gh-pages

# Delete all the files, except index.html. They will be replaced by the new
# versions.
cp index.html $TEMP_INDEX_LOCATION
git rm -rf .
mv $TEMP_LOCATION/* .
mv $TEMP_INDEX_LOCATION index.html

git add .
git commit -m "Update docs to: ${message}"
git push
git checkout -
