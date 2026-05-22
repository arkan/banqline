BINARY := banqline
CARGO  ?= cargo
ARGS   ?=

.PHONY: build run install clean

build:
	$(CARGO) build --release --bin $(BINARY)

run:
	$(CARGO) run --bin $(BINARY) -- $(ARGS)

install:
	$(CARGO) install --path . --bin $(BINARY) --force

clean:
	$(CARGO) clean
