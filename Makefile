SERVICE ?= rust-service
TOOLCHAIN ?= +1.88.0
WORKDIR ?= /apps/scurve-be
MANIFEST_PATH ?= /apps/scurve-be/Cargo.toml
TARGET_DIR ?= /apps/scurve-be/target

BASE_URL ?= https://127.0.0.1:8800
INSECURE_TLS ?= 1
CLEANUP_PROJECT ?= 1

.PHONY: help validate validate-no-smoke smoke fmt test audit deny

help:
	@echo "Targets:"
	@echo "  make validate          # smoke + fmt + tests + audit + deny"
	@echo "  make validate-no-smoke # fmt + tests + audit + deny"
	@echo "  make smoke             # runtime endpoint smoke checks"
	@echo "  make fmt               # rustfmt check in docker"
	@echo "  make test              # integration/unit tests in docker"
	@echo "  make audit             # cargo-audit in docker"
	@echo "  make deny              # cargo-deny in docker"

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
