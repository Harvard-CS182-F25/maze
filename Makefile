PY              ?= python3
MATURIN         ?= maturin
CARGO           ?= cargo
PY_CRATE        ?= Cargo.toml
BIN             ?= stub_gen
FEAT_PYMODULE   ?= pymodule
PROFILE         ?= release
CARGO_FLAGS     ?= --no-default-features
CARGO_TARGET    ?=

export PYO3_PYTHON := $(shell command -v $(PY))

.PHONY: help develop stubs wheel clean fmt test check-arch show-config

help:
	@echo "Targets:"
	@echo "  make develop        - maturin develop (installs the Python extension into current venv)"
	@echo "  make stubs          - develop the extension, then run stub generator bin"
	@echo "  make wheel          - build wheels for distribution"
	@echo "Vars (override: make VAR=value):"
	@echo "  PY=$(PY) | PY_CRATE=$(PY_CRATE) | BIN=$(BIN) | FEAT_PYMODULE=$(FEAT_PYMODULE) | PROFILE=$(PROFILE)"

develop:
	@echo ">> Using Python: $(PYO3_PYTHON)"
	$(MATURIN) develop -m $(PY_CRATE) $(RELEASE_FLAG) $(if $(FEAT_PYMODULE),--features $(FEAT_PYMODULE),)

stubs: develop
	@echo ">> Running stub generator bin: $(BIN)"
	$(CARGO) run $(CARGO_TARGET) --bin $(BIN) $(CARGO_FLAGS)

wheel:
	@echo ">> Building wheels via maturin"
	$(MATURIN) build -m $(PY_CRATE) $(RELEASE_FLAG) $(if $(FEAT_PYMODULE),--features $(FEAT_PYMODULE),)

clean:
	$(CARGO) clean
	@rm -rf target/wheels || true