#!/usr/bin/env bash
set -eu

echo "##### Building test book #####"
rm -rf test-book/book
mdbook build test-book

echo
echo "##### Comparing book results (no output is correct) #####"
diff -rw test-book/expected test-book/book
