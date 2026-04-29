SERVICE ?= rust-service
TOOLCHAIN ?= +1.88.0
WORKDIR ?= /apps/scurve-be
MANIFEST_PATH ?= /apps/scurve-be/Cargo.toml
TARGET_DIR ?= /apps/scurve-be/target
APP_PORT ?= 8800

BASE_URL ?= https://127.0.0.1:$(APP_PORT)
INSECURE_TLS ?= 1
CLEANUP_PROJECT ?= 1

.PHONY: help validate validate-no-smoke smoke fmt test audit deny \
        openapi run run-openapi run-release kill \
        migrate migrate-status

help:
	@echo "Targets:"
	@echo "  make validate          # smoke + fmt + tests + audit + deny"
	@echo "  make validate-no-smoke # fmt + tests + audit + deny"
	@echo "  make smoke             # runtime endpoint smoke checks"
	@echo "  make fmt               # rustfmt check in docker"
	@echo "  make test              # integration/unit tests in docker"
	@echo "  make audit             # cargo-audit in docker"
	@echo "  make deny              # cargo-deny in docker"
	@echo "  make migrate           # apply pending migrations"
	@echo "  make migrate-status    # compare applied vs pending migrations"
	@echo "  make openapi           # regenerate openapi.json"
	@echo "  make kill              # stop the running API server in the container"
	@echo "  make run               # kill + start API server (local-release)"
	@echo "  make run-openapi       # kill + regenerate openapi.json + start API server"
	@echo "  make run-release       # kill + start API server (staging profile)"

validate: smoke fmt test audit deny

validate-no-smoke: fmt test audit deny

smoke:
	BASE_URL="$(BASE_URL)" INSECURE_TLS="$(INSECURE_TLS)" CLEANUP_PROJECT="$(CLEANUP_PROJECT)" ./scripts/smoke_api.sh

fmt:
	docker exec -w $(WORKDIR) $(SERVICE) cargo $(TOOLCHAIN) fmt --manifest-path $(MANIFEST_PATH) --all --check

test:
	docker exec -w $(WORKDIR) $(SERVICE) cargo $(TOOLCHAIN) test --manifest-path $(MANIFEST_PATH) --target-dir $(TARGET_DIR) --tests

audit:
	docker exec -w $(WORKDIR) $(SERVICE) cargo $(TOOLCHAIN) audit

deny:
	docker exec -w $(WORKDIR) $(SERVICE) cargo $(TOOLCHAIN) deny check

openapi:
	docker exec -w $(WORKDIR) $(SERVICE) cargo $(TOOLCHAIN) run --manifest-path $(MANIFEST_PATH) --target-dir $(TARGET_DIR) --release --bin dump_openapi

kill:
	-docker exec $(SERVICE) sh -c 'fuser -k $(APP_PORT)/tcp 2>/dev/null || pkill -f "/apps/scurve-be/target" 2>/dev/null; sleep 1; true'

run: kill
	docker exec $(SERVICE) env RUST_BACKTRACE=1 \
		CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem \
		cargo $(TOOLCHAIN) run --manifest-path $(MANIFEST_PATH) \
		--target-dir $(TARGET_DIR) --profile local-release

run-release: kill
	docker exec $(SERVICE) env RUST_BACKTRACE=1 \
		CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem \
		cargo $(TOOLCHAIN) run --manifest-path $(MANIFEST_PATH) \
		--target-dir $(TARGET_DIR) --profile staging

run-openapi: openapi run

migrate:
	docker exec -w $(WORKDIR) $(SERVICE) cargo $(TOOLCHAIN) run \
		--manifest-path $(MANIFEST_PATH) --target-dir $(TARGET_DIR) \
		--release --bin cli -- migrate-run

migrate-status:
	docker exec -w $(WORKDIR) $(SERVICE) cargo $(TOOLCHAIN) run \
		--manifest-path $(MANIFEST_PATH) --target-dir $(TARGET_DIR) \
		--release --bin cli -- migrate-status
