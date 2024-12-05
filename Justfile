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
on-chain-test test="all": setup-test-suite setup-node
    @if [ "{{ test }}" = "all" ]; then \
        echo "Running all tests except those with prefix 'test-suite-sanity'..."; \
        for file in scripts/*.test.ts; do \
            if ! echo $file | grep -q '^scripts/test-suite-sanity'; then \
                echo "Running test $$file..."; \
                bun test $file; \
            fi; \
        done; \
    else \
        bun test "scripts/{{ test }}.test.ts"; \
    fi

# Build all contracts and optimize WASM artifacts
build-contracts:
    #!/usr/bin/env nu
    mkdir artifacts;
    # Find contract packages
    rg --files contracts --glob Cargo.toml
    | lines
    | par-each {
        open
        | get package.name
        | do {
            let pkg = $in
            print $"Compiling ($pkg) wasm..."
            # Compile wasm artifact
            RUSTFLAGS="-C link-arg=-s" cargo build --quiet --package $pkg --lib --release --target wasm32-unknown-unknown;
            # Optimize wasm artifact
            let opt_in = $"target/wasm32-unknown-unknown/release/($pkg | str snake-case).wasm";
            let opt_out = $"artifacts/($pkg).wasm";
            wasm-opt -Os --signext-lowering $opt_in -o $opt_out;
            $opt_out
        }
    };
    cd artifacts;
    # Generate checksum for WASM artifacts
    sha256sum *.wasm | save -f checksum.txt;
    # Show files and sizes
    ls ./ | select name size | rename artifact | table --theme light -i false | print $"\n($in)\n"

# Generate all JSON schemas for the contract interfaces
generate-schemas:
    #!/usr/bin/env nu
    mkdir schema;
    let schema_dir = $"(pwd)/schema";
    # Find contract packages
    rg --files contracts --glob Cargo.toml
    | lines
    | each { open }
    | filter { $in.bin? | any { $in.name | str contains "schema" } }
    | par-each {
        get package.name
        | do {
            print $"Generating schema bindings for ($in)"
            let tempdir = $"target/contracts/($in)";
            let outdir = $"($schema_dir)/($in)";
            mkdir $tempdir;
            cd $tempdir;
            cargo run --quiet --package $in --bin $"($in)-schema";
            rm -rf $outdir;
            cp -rf ./schema $outdir;
        }
    };
    rm -rf target/schemas;
    # Show result
    ls schema | select name | rename schema_dirs | table --theme light -i false | print $"\n($in)\n"

# Generate TypeScript bindings for all the contract messages
generate-ts: setup-node
    #!/usr/bin/env nu
    # Find contract packages
    rg --files contracts --glob Cargo.toml
    | lines
    | each { open }
    | filter { $in.bin? | any { $in.name | str contains "schema" } }
    | par-each {
        get package.name
        | do {
            print $"Generating typescript bindings for ($in)";
            let contract = ($in | split words | each { str capitalize } | str join);
            let schema_dir = $"./schema/($in)";
            NODE_NO_WARNINGS=1 (npx @cosmwasm/ts-codegen generate 
                --schema $schema_dir 
                --out ./ts 
                --name $contract 
                --plugin client 
                --no-bundle)
        }
    };
    # Show result
    ls ts | select name | rename bindings | table --theme light -i false | print $"\n($in)\n"

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
export GAIA_VERSION        := env("GAIA_VERSION", "v21.0.0")
export GAIA_IMAGE          := "cosmos/gaia:" + GAIA_VERSION
export NEUTRON_VERSION     := env("NEUTRON_VERSION", "v5.0.2")
export NEUTRON_IMAGE       := "neutron-org/neutron:" + NEUTRON_VERSION

# Setup on-chain test suite
setup-test-suite:
    @if ! docker image inspect $ICQ_RELAYER_IMAGE > /dev/null 2>&1; then \
        echo "Build {{ ICQ_RELAYER_IMAGE }}"; \
        mkdir -p target/test-suite; \
        cd target/test-suite; \
        rm -rf neutron-query-relayer; \
        git clone --depth 1 --branch $ICQ_RELAYER_VERSION https://github.com/neutron-org/neutron-query-relayer; \
        cd neutron-query-relayer; \
        docker build . -t $ICQ_RELAYER_IMAGE; \
    fi
    @if ! docker image inspect $HERMES_IMAGE > /dev/null 2>&1; then \
        echo "Build {{ HERMES_IMAGE }}"; \
        mkdir -p target/test-suite; \
        cd target/test-suite; \
        rm -rf hermes; \
        git clone --depth 1 --branch $HERMES_VERSION https://github.com/informalsystems/hermes; \
        cd hermes; \
        sed -i '/^ARG UID=/d' ci/release/hermes.Dockerfile; \
        sed -i '/^ARG GID=/d' ci/release/hermes.Dockerfile; \
        sed -i '/^RUN groupadd -g \${GID} hermes && useradd -l -m hermes -s \/bin\/bash -u \${UID} -g \${GID}/d' ci/release/hermes.Dockerfile; \
        sed -i '/^USER hermes:hermes/d' ci/release/hermes.Dockerfile; \
        sed -i 's/--chown=hermes:hermes[[:space:]]*//' ci/release/hermes.Dockerfile; \
        docker build -t $HERMES_IMAGE -f ci/release/hermes.Dockerfile .; \
    fi
    @if ! docker image inspect $GAIA_IMAGE > /dev/null 2>&1; then \
        echo "Build {{ GAIA_IMAGE }}"; \
        mkdir -p target/test-suite; \
        cd target/test-suite; \
        rm -rf gaia; \
        git clone --depth 1 --branch $GAIA_VERSION https://github.com/cosmos/gaia; \
        cd gaia; \
        sed -i '/RUN addgroup -g 1025 nonroot/d' Dockerfile; \
        sed -i '/RUN adduser -D nonroot -u 1025 -G nonroot/d' Dockerfile; \
        sed -i '/^USER nonroot/d' Dockerfile; \
        docker build -t $GAIA_IMAGE -f Dockerfile .; \
    fi
    @if ! docker image inspect $NEUTRON_IMAGE > /dev/null 2>&1; then \
        echo "Build {{ NEUTRON_IMAGE }}"; \
        mkdir -p target/test-suite; \
        cd target/test-suite; \
        rm -rf neutron; \
        git clone --depth 1 --branch $NEUTRON_VERSION https://github.com/neutron-org/neutron; \
        cd neutron; \
        sed -i '/^CMD bash \/opt\/neutron\/network\/init.sh && \\/d' Dockerfile; \
        sed -i '/^    bash \/opt\/neutron\/network\/init-neutrond.sh && \\/d' Dockerfile; \
        sed -i '/^    bash \/opt\/neutron\/network\/start.sh$/d' Dockerfile; \
        echo 'ENTRYPOINT ["neutrond"]' >> Dockerfile; \
        docker buildx build --load --build-context app=. -t $NEUTRON_IMAGE --build-arg BINARY=neutrond .; \
    fi


# Clean on-chain test suite
clean-test-suite:
    @if docker image inspect $ICQ_RELAYER_IMAGE > /dev/null 2>&1; then \
        docker rmi -f $ICQ_RELAYER_IMAGE; \
    fi
    @if docker image inspect $HERMES_IMAGE > /dev/null 2>&1; then \
        docker rmi -f $HERMES_IMAGE; \
    fi
    @if docker image inspect $GAIA_IMAGE > /dev/null 2>&1; then \
        docker rmi -f $GAIA_IMAGE; \
    fi
    @if docker image inspect $NEUTRON_IMAGE > /dev/null 2>&1; then \
        docker rmi -f $NEUTRON_IMAGE; \
    fi
