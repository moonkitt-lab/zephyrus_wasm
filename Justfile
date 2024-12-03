# Default task to build all contracts and generate schemas/TS bindings
dist: lint check-fmt clean-artifacts build-contracts generate-schemas generate-ts

# Show all available tasks
menu:
    @just --list

setup-node:
    @if [ ! -d "node_modules" ]; then \
        echo "node_modules directory not found. Running npm install..."; \
        NODE_NO_WARNINGS=1 npm install; \
    else \
        echo "node_modules directory exists. Skipping npm install."; \
    fi

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

