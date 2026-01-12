.PHONY: core-test core-fmt core-fmt-check core-lint help migrate python-docs workflow-docs lsp-install lsp-dev

help:
	@echo "Available targets:"
	@echo "  core-test      Run tests for the Rust core library"
	@echo "  core-fmt       Fix Rust formatting"
	@echo "  core-fmt-check Check Rust formatting (for CI)"
	@echo "  core-lint      Run clippy linter"
	@echo "  migrate        Run database migrations"
	@echo "  python-docs    Generate Python API documentation (YAML + Markdown)"
	@echo "  workflow-docs  Generate Workflow API documentation (Markdown)"
	@echo "  lsp-install    Install rhythm-lsp to ~/.local/bin"
	@echo "  lsp-dev        Set up VS Code extension for local development"

db:
	docker compose up -d

db-reset:
	docker compose down -v
	docker compose up -d

migrate:
	cd core && RHYTHM_DATABASE_URL=postgresql://rhythm@localhost/rhythm cargo run --release --bin rhythm -- migrate

core-test:
	cd core && cargo test

core-fmt:
	cd core && cargo fmt

core-fmt-check:
	cd core && cargo fmt --check

core-lint:
	cd core && cargo clippy --all-targets --all-features -- -D warnings

core-ci:
	$(MAKE) core-fmt
	$(MAKE) core-lint
	$(MAKE) core-test

python-docs:
	python/.venv/bin/python python/scripts/generate_api_ref.py
	python/.venv/bin/python docs/gen/render_api_docs.py python/docs/python-api.yml docs/python_reference.md

workflow-docs:
	python/.venv/bin/python docs/gen/render_api_docs.py core/docs/workflow-api.yml docs/workflow_reference.md

lsp-install:
	./editors/scripts/install-lsp.sh

lsp-dev:
	./editors/scripts/dev-vscode.sh