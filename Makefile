#!/usr/bin/make -f

PACKAGES_SIMTEST=$(shell go list ./... | grep '/simulation')
#VERSION := $(shell echo $(shell git describe --always) | sed 's/^v//')
VERSION := v0.3.1
COMMIT := $(shell git log -1 --format='%H')
LEDGER_ENABLED ?= false
GOMOD_FLAGS ?=
SDK_PACK := $(shell go list -m github.com/cosmos/cosmos-sdk | sed  's/ /\@/g')

# for dockerized protobuf tools
HTTPS_GIT := https://github.com/oraichain/orai.git

export GO111MODULE = on

# process build tags

build_tags = netgo
ifeq ($(LEDGER_ENABLED),true)
  ifeq ($(OS),Windows_NT)
    GCCEXE = $(shell where gcc.exe 2> NUL)
    ifeq ($(GCCEXE),)
      $(error gcc.exe not installed for ledger support, please install or set LEDGER_ENABLED=false)
    else
      build_tags += ledger
    endif
  else
    UNAME_S = $(shell uname -s)
    ifeq ($(UNAME_S),OpenBSD)
      $(warning OpenBSD detected, disabling ledger support (https://github.com/cosmos/cosmos-sdk/issues/1988))
    else
      GCC = $(shell command -v gcc 2> /dev/null)
      ifeq ($(GCC),)
        $(error gcc not installed for ledger support, please install or set LEDGER_ENABLED=false)
      else
        build_tags += ledger
      endif
    endif
  endif
endif

ifeq ($(WITH_CLEVELDB),yes)
  build_tags += gcc
endif
build_tags += $(BUILD_TAGS)
build_tags := $(strip $(build_tags))

whitespace := 
empty = $(whitespace) $(whitespace)
comma := ,
build_tags_comma_sep := $(subst $(empty),$(comma),$(build_tags))

# process linker flags
ldflags = -X github.com/cosmos/cosmos-sdk/version.Name=orai \
		  -X github.com/cosmos/cosmos-sdk/version.AppName=oraid \
		  -X github.com/cosmos/cosmos-sdk/version.Version=$(VERSION) \
		  -X github.com/cosmos/cosmos-sdk/version.Commit=$(COMMIT) \
		  -X github.com/oraichain/orai/app.Bech32Prefix=orai \
		  -X "github.com/cosmos/cosmos-sdk/version.BuildTags=$(build_tags_comma_sep)"

ifeq ($(WITH_CLEVELDB),yes)
  ldflags += -X github.com/cosmos/cosmos-sdk/types.DBBackend=cleveldb
endif
ldflags += $(LDFLAGS)
ldflags := $(strip $(ldflags))

BUILD_FLAGS := -tags "$(build_tags_comma_sep)" -ldflags '-s -w $(ldflags)' -trimpath


all: install lint test

watch:
	air -c oraid.toml

build:
	BUILD_TAGS=muslc make go-build
	cp build/oraid /bin

go-build: go.sum
ifeq ($(OS),Windows_NT)
	exit 1
else
	go build $(GOMOD_FLAGS) $(BUILD_FLAGS) -o build/oraid ./cmd/oraid
endif

build-contract-tests-hooks:
ifeq ($(OS),Windows_NT)
	go build $(GOMOD_FLAGS) $(BUILD_FLAGS) -o build/contract_tests.exe ./cmd/contract_tests
else
	go build $(GOMOD_FLAGS) $(BUILD_FLAGS) -o build/contract_tests ./cmd/contract_tests
endif

test-method:
	BUILD_TAGS=muslc make go-test-method

go-test-method:
	go test $(GOMOD_FLAGS) $(BUILD_FLAGS) -run $(METHOD) $(PACKAGE) -v

install: go.sum
	go install $(GOMOD_FLAGS) $(BUILD_FLAGS) ./cmd/oraid

########################################
### Tools & dependencies

go-mod-cache: go.sum
	@echo "--> Download go modules to local cache"
	@go mod download

go.sum: go.mod
	@echo "--> Ensure dependencies have not been modified"
	@go mod verify

draw-deps:
	@# requires brew install graphviz or apt-get install graphviz
	go get github.com/RobotsAndPencils/goviz
	@goviz -i ./cmd/oraid -d 2 | dot -Tpng -o dependency-graph.png

clean:
	rm -rf snapcraft-local.yaml build/

distclean: clean
	rm -rf vendor/

########################################
### Testing


test: test-unit
test-all: check test-race test-cover

test-unit:
	@VERSION=$(VERSION) go test $(GOMOD_FLAGS) -tags='ledger test_ledger_mock' ./...

test-race:
	@VERSION=$(VERSION) go test $(GOMOD_FLAGS) -race -tags='ledger test_ledger_mock' ./...

test-cover:
	@go test $(GOMOD_FLAGS) -timeout 30m -race -coverprofile=coverage.txt -covermode=atomic -tags='ledger test_ledger_mock' ./...


benchmark:
	@go test $(GOMOD_FLAGS) -bench=. ./...


###############################################################################
###                                Linting                                  ###
###############################################################################

lint:
	golangci-lint run
	find . -name '*.go' -type f -not -path "./vendor*" -not -path "*.git*" | xargs gofmt -d -s

format:
	find . -name '*.go' -type f -not -path "./vendor*" -not -path "*.git*" -not -path "./client/lcd/statik/statik.go" | xargs gofmt -w -s
	find . -name '*.go' -type f -not -path "./vendor*" -not -path "*.git*" -not -path "./client/lcd/statik/statik.go" | xargs misspell -w
	find . -name '*.go' -type f -not -path "./vendor*" -not -path "*.git*" -not -path "./client/lcd/statik/statik.go" | xargs goimports -w -local github.com/oraichain/orai


###############################################################################
###                                Protobuf                                 ###
###############################################################################


proto-all: proto-gen proto-check-breaking
.PHONY: proto-all

proto-gen: 
	./scripts/protocgen.sh $(PROTO_DIR)
.PHONY: proto-gen

proto-js: 
	./scripts/protocgen-js.sh $(SRC_DIR)
.PHONY: proto-js

proto-swagger: 
	./scripts/protocgen-swagger.sh $(SRC_DIR)
.PHONY: proto-swagger

proto-check-breaking:
	buf check breaking --against-input $(HTTPS_GIT)#branch=master
.PHONY: proto-check-breaking

.PHONY: all build-linux install install-debug \
	go-mod-cache draw-deps clean build format \
	test test-all test-build test-cover test-unit test-race

