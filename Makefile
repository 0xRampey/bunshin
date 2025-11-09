.PHONY: build plugin cli clean run install test help

# Default target
all: build

# Full build
build:
	@./dev.sh build

# Build only plugin (fast iteration)
plugin:
	@./dev.sh plugin

# Build only CLI
cli:
	@./dev.sh cli

# Clean all build artifacts
clean:
	@./dev.sh clean

# Run the built binary
run:
	@./dev.sh run

# Install to ~/.cargo/bin
install:
	@./dev.sh install

# Run tests
test:
	@./dev.sh test

# Show help
help:
	@./dev.sh help
