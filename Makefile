.PHONY: build run release tarball test

build:
	CARGO_BUILD_JOBS=8 cargo build

run: build
	./target/debug/clean

release:
	CARGO_BUILD_JOBS=8 cargo build --release --locked

test:
	cargo test

# Produce the source archive expected by packaging/PKGBUILD.
tarball:
	tar --exclude='./target' --exclude='./.git' --exclude='./packaging' \
	    --transform 's,^\./,clean-0.1.0/,' \
	    -czf /tmp/opencode/clean-0.1.0.tar.gz .
	mv /tmp/opencode/clean-0.1.0.tar.gz clean-0.1.0.tar.gz
