.PHONY: test-core help migrate python-docs workflow-docs

help:
	@echo "Available targets:"
	@echo "  test-core      Run tests for the Rust core library"
	@echo "  migrate        Run database migrations"
	@echo "  python-docs    Generate Python API documentation (YAML + Markdown)"
	@echo "  workflow-docs  Generate Workflow API documentation (Markdown)"

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

python-docs:
	python/.venv/bin/python python/scripts/generate_api_ref.py
	python/.venv/bin/python docs/gen/render_api_docs.py python/docs/python-api.yml docs/python-api.md

workflow-docs:
	python/.venv/bin/python docs/gen/render_api_docs.py docs/workflow-api.yml docs/workflow-api.md