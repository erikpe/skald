CC ?= cc
AR ?= ar
CFLAGS ?= -O2
CFLAGS += -std=c11 -Wall -Wextra -Werror -Iinclude

BUILD_DIR ?= ../build/runtime
OBJECT := $(BUILD_DIR)/skald_runtime.o
ARCHIVE := $(BUILD_DIR)/libskald_runtime.a
TEST_SOURCE := ../tests/runtime/test_runtime_abi.c
TEST_BINARY := $(BUILD_DIR)/test_runtime_abi

.PHONY: all test clean

all: $(ARCHIVE)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(OBJECT): src/skald_runtime.c include/skald_runtime.h | $(BUILD_DIR)
	$(CC) $(CFLAGS) -c src/skald_runtime.c -o $(OBJECT)

$(ARCHIVE): $(OBJECT)
	$(AR) rcs $(ARCHIVE) $(OBJECT)

$(TEST_BINARY): $(TEST_SOURCE) $(ARCHIVE) include/skald_runtime.h
	$(CC) $(CFLAGS) $(TEST_SOURCE) $(ARCHIVE) -o $(TEST_BINARY)

test: $(TEST_BINARY)
	$(TEST_BINARY)

clean:
	rm -f $(OBJECT) $(ARCHIVE) $(TEST_BINARY)

