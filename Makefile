.PHONY: test-core help migrate

help:
	@echo "Available targets:"
	@echo "  test-core    Run tests for the Rust core library"
	@echo "  migrate      Run database migrations"

init:
	./init.sh

db:
	docker compose up -d

db-reset:
	docker compose down -v
	docker compose up -d

migrate:
	cd core && RHYTHM_DATABASE_URL=postgresql://postgres@localhost/rhythm cargo run --release --bin rhythm -- migrate

core-test:
	cd core && cargo test -- --test-threads=1
