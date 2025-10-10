set unstable

# Default task to build all contracts and generate schemas/TS bindings
dist: lint check-fmt clean-artifacts build-contracts generate-schemas generate-ts

# Show all available tasks
menu:
    @just --list

# Remove all build artifacts
clean-artifacts:
    @echo "Cleaning previous artifacts..."
    rm -rf artifacts schema ts

# Run linter
lint:
    @echo "Linting with clippy..."
    cargo clippy --quiet

# Check code formatting
check-fmt:
    @echo "Checking formatting..."
    cargo fmt --check

# Run on-chain tests locally
[script]
on-chain-test test="all": setup-test-suite setup-node
    if [ "{{ test }}" = "all" ]; then
        echo "Running all tests except those with prefix 'test-suite-sanity'..."
        for file in scripts/*.test.ts; do
            if ! echo $file | grep -q '^scripts/test-suite-sanity'; then
                echo "Running test $file..."
                bun test $file --timeout 1600000 --bail 1
            fi
        done
    else
        bun test "scripts/{{ test }}.test.ts" --timeout 1600000 --bail 1
    fi

[script]
build-contracts:
    mkdir -p artifacts;
    find contracts -name "Cargo.toml" | while read -r file; do
        pkg=$(awk '/^\[package\]/ {in_package=1} /^\[\[/ {in_package=0} in_package && /^name = / {print $3}' "$file" | tr -d '"')
        if [ -n "$pkg" ]; then
            echo "Compiling $pkg wasm..."
            RUSTFLAGS="-C link-arg=-s" cargo build --quiet --package "$pkg" --lib --release --target wasm32-unknown-unknown
            opt_in="target/wasm32-unknown-unknown/release/$(echo "$pkg" | sed -E 's/-/_/g').wasm"
            opt_out="artifacts/$pkg.wasm"
            echo "Optimizing $opt_in..."
            wasm-opt -Os --signext-lowering "$opt_in" -o "$opt_out"
        fi
    done
    cd artifacts
    sha256sum *.wasm > checksum.txt
    echo ""
    echo -n "Artifacts:"
    ls -lh | awk '{printf "%-30s %10s\n", $9, $5}'
    echo ""

# Generate all JSON schemas for the contract interfaces
[script]
generate-schemas:
    mkdir -p schema
    schema_dir=$(pwd)/schema

    find contracts -name "Cargo.toml" | while read -r file; do
        pkg=$(awk '/^\[package\]/ {in_package=1} /^\[\[/ {in_package=0} in_package && /^name = / {print $3}' "$file" | tr -d '"')
        bin_name=$(awk '/^\[\[bin\]\]/ {in_bin=1} /^\[/ && $1 != "[[bin]]" {in_bin=0} in_bin && /^name = / {print $3}' "$file" | grep -o 'schema' || true)

        if [ -n "$pkg" ] && [ -n "$bin_name" ]; then
            echo "Generating schema bindings for $pkg"
            tempdir="target/contracts/$pkg"
            outdir="$schema_dir/$pkg"
            mkdir -p "$tempdir"
            cd "$tempdir"
            cargo run --quiet --package "$pkg" --bin "$pkg-schema"
            rm -rf "$outdir"
            cp -rf ./schema "$outdir"
        fi
    done
    rm -rf target/schemas
    echo ""
    echo "Schema Directories:"
    ls -1 schema
    echo ""

# Generate TypeScript bindings for all the contract messages
[script]
generate-ts: setup-node
    echo "Generating TypeScript bindings for contract schemas..."
    bun run scripts/generate-ts-bindings.js
    echo ""
    echo "TypeScript Bindings:"
    ls -1 ts
    echo ""

# Setup typescript 
setup-node:
    @echo "Checking node dependencies are up to date..."
    @bun install

# Clean NodeJS
clean-node:
    rm -rf node_modules

export ICQ_RELAYER_VERSION := env("ICQ_RELAYER_VERSION", "v0.3.0")
export ICQ_RELAYER_IMAGE   := "neutron-org/neutron-query-relayer:" + ICQ_RELAYER_VERSION
export HERMES_VERSION      := env("HERMES_VERSION", "v1.10.4")
export HERMES_IMAGE        := "informalsystems/hermes:" + HERMES_VERSION
export GAIA_VERSION        := env("GAIA_VERSION", "v21.0.1")
export GAIA_IMAGE          := "cosmos/gaia:" + GAIA_VERSION
export NEUTRON_VERSION     := env("NEUTRON_VERSION", "v5.0.4")
export NEUTRON_IMAGE       := "neutron-org/neutron:" + NEUTRON_VERSION

export HYDRO_VERSION       := env("HYDRO_VERSION", "v3.5.0")

# Setup on-chain test suite
[script]
setup-test-suite:
    root_dir=`pwd`
    if ! docker image inspect $ICQ_RELAYER_IMAGE > /dev/null 2>&1; then
        echo "Build {{ ICQ_RELAYER_IMAGE }}"
        mkdir -p target/test-suite
        cd target/test-suite
        rm -rf neutron-query-relayer
        git clone --depth 1 --branch $ICQ_RELAYER_VERSION https://github.com/neutron-org/neutron-query-relayer
        cd neutron-query-relayer
        docker build . -t $ICQ_RELAYER_IMAGE
        cd $root_dir
    fi
    if ! docker image inspect $HERMES_IMAGE > /dev/null 2>&1; then
        echo "Build {{ HERMES_IMAGE }}"
        mkdir -p target/test-suite
        cd target/test-suite
        rm -rf hermes
        git clone --depth 1 --branch $HERMES_VERSION https://github.com/informalsystems/hermes
        cd hermes
        sed -i '/^ARG UID=/d' ci/release/hermes.Dockerfile
        sed -i '/^ARG GID=/d' ci/release/hermes.Dockerfile
        sed -i '/^RUN groupadd -g \${GID} hermes && useradd -l -m hermes -s \/bin\/bash -u \${UID} -g \${GID}/d' ci/release/hermes.Dockerfile
        sed -i '/^USER hermes:hermes/d' ci/release/hermes.Dockerfile
        sed -i 's/--chown=hermes:hermes[[:space:]]*//' ci/release/hermes.Dockerfile
        docker build -t $HERMES_IMAGE -f ci/release/hermes.Dockerfile .
        cd $root_dir
    fi
    if ! docker image inspect $GAIA_IMAGE > /dev/null 2>&1; then
        echo "Build {{ GAIA_IMAGE }}"
        mkdir -p target/test-suite
        cd target/test-suite
        rm -rf gaia
        git clone --depth 1 --branch $GAIA_VERSION https://github.com/cosmos/gaia
        cd gaia
        sed -i '/RUN addgroup -g 1025 nonroot/d' Dockerfile
        sed -i '/RUN adduser -D nonroot -u 1025 -G nonroot/d' Dockerfile
        sed -i '/^USER nonroot/d' Dockerfile
        docker build -t $GAIA_IMAGE -f Dockerfile .
        cd $root_dir
    fi
    if ! docker image inspect $NEUTRON_IMAGE > /dev/null 2>&1; then
        echo "Build {{ NEUTRON_IMAGE }}"
        mkdir -p target/test-suite
        cd target/test-suite
        rm -rf neutron
        git clone --depth 1 --branch $NEUTRON_VERSION https://github.com/neutron-org/neutron
        cd neutron
        sed -i '/^CMD bash \/opt\/neutron\/network\/init.sh && \\/d' Dockerfile
        sed -i '/^    bash \/opt\/neutron\/network\/init-neutrond.sh && \\/d' Dockerfile
        sed -i '/^    bash \/opt\/neutron\/network\/start.sh$/d' Dockerfile
        echo 'ENTRYPOINT ["neutrond"]' >> Dockerfile
        docker buildx build --load --build-context app=. -t $NEUTRON_IMAGE --build-arg BINARY=neutrond .
        cd $root_dir
    fi
    hydro_dir="target/test-suite/hydro/$HYDRO_VERSION"
    if [ ! -d $hydro_dir ]; then
        mkdir -p target/test-suite/hydro
        cd target/test-suite/hydro
        git clone --depth 1 --branch $HYDRO_VERSION https://github.com/informalsystems/hydro $HYDRO_VERSION
        cd $root_dir
    fi 


# Clean on-chain test suite
[script]
clean-test-suite:
    if docker image inspect $ICQ_RELAYER_IMAGE > /dev/null 2>&1; then
        docker rmi -f $ICQ_RELAYER_IMAGE
    fi
    if docker image inspect $HERMES_IMAGE > /dev/null 2>&1; then
        docker rmi -f $HERMES_IMAGE
    fi
    if docker image inspect $GAIA_IMAGE > /dev/null 2>&1; then
        docker rmi -f $GAIA_IMAGE
    fi
    if docker image inspect $NEUTRON_IMAGE > /dev/null 2>&1; then
        docker rmi -f $NEUTRON_IMAGE
    fi
    rm -rf target/test-suite
