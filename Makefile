.PHONY: test-core help

help:
	@echo "Available targets:"
	@echo "  test-core    Run tests for the Rust core library"

init:
	./init.sh

db:
	docker compose up -d

db-reset:
	docker compose down -v
	docker compose up -d

core-test:
	cd core && cargo test -- --test-threads=1
