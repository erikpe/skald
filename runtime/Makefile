CC ?= cc
AR ?= ar
CFLAGS ?= -O2
CFLAGS += -std=c11 -Wall -Wextra -Werror -Iinclude

BUILD_DIR ?= ../build/runtime
OBJECT := $(BUILD_DIR)/skald_runtime.o
ARCHIVE := $(BUILD_DIR)/libskald_runtime.a

.PHONY: all clean

all: $(ARCHIVE)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(OBJECT): src/skald_runtime.c include/skald_runtime.h | $(BUILD_DIR)
	$(CC) $(CFLAGS) -c src/skald_runtime.c -o $(OBJECT)

$(ARCHIVE): $(OBJECT)
	$(AR) rcs $(ARCHIVE) $(OBJECT)

clean:
	rm -f $(OBJECT) $(ARCHIVE)

