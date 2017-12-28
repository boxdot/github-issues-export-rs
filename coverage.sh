set -ex

# get and build kcov
wget https://github.com/SimonKagstrom/kcov/archive/master.tar.gz
tar xzf master.tar.gz
cd kcov-master
mkdir build
cd build
cmake ..
make
make install DESTDIR=../../kcov-build
cd ../..
rm -rf kcov-master

# run kcov and upload stats to coveralls
mkdir -p "target/cov/github-issues-export"
./kcov-build/usr/local/bin/kcov \
    --coveralls-id=$TRAVIS_JOB_ID \
    --exclude-pattern=/.cargo,/usr/lib \
    --verify "target/cov/github-issues-export" "target/debug/github-issues-export";

echo "Uploaded code coverage to Coveralls"
