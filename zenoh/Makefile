#
# Copyright (c) 2017, 2020 ADLINK Technology Inc.
#
# This program and the accompanying materials are made available under the
# terms of the Eclipse Public License 2.0 which is available at
# http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
# which is available at https://www.apache.org/licenses/LICENSE-2.0.
#
# SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
#
# Contributors:
#   ADLINK zenoh team, <zenoh@adlink-labs.tech>
#

# WARNING: this Makefile has the following assumptions:
#  - The host is on MacOS
#  - JAVA_HOME environment variable points to the JDK installation dir
#  - swig is installed
#  - zenoh-c source directory is present in the same directory than zenoh-java

TARGET_DIR:=target

ifdef ZENOHC_HOME
ZENOHC_DEV_DIR := $(ZENOHC_HOME)
else
ZENOHC_DEV_DIR := ../../zenoh-c
endif

ZENOHC_COPY_DIR := $(TARGET_DIR)/zenoh-c
ZENOHC_BUILD_DIR := $(ZENOHC_COPY_DIR)/build
ZENOHC_LIB_NAME := zenohc_java


SWIG_FILE := src/main/swig/zenohc.i
SWIG_JAVA_OUTDIR := $(TARGET_DIR)/generated-sources/java/org/eclipse/zenoh/swig
SWIG_C_OUTDIR := $(ZENOHC_COPY_DIR)/src
SWIG_C_FILE := $(SWIG_C_OUTDIR)/zenohc_java.c

JNI_INCLUDE_ALL_PLATFORMS := $(ZENOHC_COPY_DIR)/jni-include
NATIVES_DIR := $(TARGET_DIR)/resources/natives


# CMAKE_OPTIONS := -DSWIG_JAVA=ON -DTESTS=OFF -DEXAMPLES=OFF
# ifneq ($(ZENOH_DEBUG),)
# 	CMAKE_OPTIONS += -DZENOH_DEBUG=$(ZENOH_DEBUG)
# endif
# ifneq ($(CMAKE_BUILD_TYPE),)
# 	CMAKE_OPTIONS += -DCMAKE_BUILD_TYPE=$(CMAKE_BUILD_TYPE)
# endif

# Naive OS detection... TODO: probably to be improved
UNAME_S := $(shell uname -s)
UNAME_M := $(shell uname -m)

ifeq ($(UNAME_S),Darwin)
	OS := osx
	LOCAL_LIB_NAME := lib$(ZENOHC_LIB_NAME).dylib
else ifeq ($(UNAME_S),$(filter $(UNAME_S),Linux))
	OS := linux
	LOCAL_LIB_NAME := lib$(ZENOHC_LIB_NAME).so
else ifeq ($(UNAME_S),$(filter $(UNAME_S),Windows_NT CYGWIN_NT MINGW))
	OS := windows
	LOCAL_LIB_NAME := $(ZENOHC_LIB_NAME).dll
else
	$(error "Unsupported platform. (uname -s = $(UNAME_S)")
endif

ifeq ($(UNAME_M),$(filter $(UNAME_M),i686 i386))
	ARCH := 32
else ifeq ($(UNAME_M),$(filter $(UNAME_M),x86_64 amd64 ))
	ARCH := 64
else
	$(error "Unsupported architecture. (uname -m = $(UNAME_M)")
endif

LOCAL_LIB := $(ZENOHC_BUILD_DIR)/$(LOCAL_LIB_NAME)


all: $(LOCAL_LIB)
	@mkdir -p $(NATIVES_DIR)/$(OS)_$(ARCH)
	@cp $(LOCAL_LIB) $(NATIVES_DIR)/$(OS)_$(ARCH)/$(LOCAL_LIB_NAME)
	
all-cross: $(ZENOHC_COPY_DIR)/Makefile $(SWIG_C_FILE) $(JNI_INCLUDE_ALL_PLATFORMS)/jni.h
	ZENOH_JAVA=ON JNI_INCLUDE_HOME=jni-include make -C$(ZENOHC_COPY_DIR) manylinux2010-x86 manylinux2010-x64 linux-armv6 linux-arm64 osx-64
	@mkdir -p $(NATIVES_DIR)/linux_32
	@cp $(ZENOHC_BUILD_DIR)/crossbuilds/manylinux2010-x86/lib* $(NATIVES_DIR)/linux_32
	@mkdir -p $(NATIVES_DIR)/linux_64
	@cp $(ZENOHC_BUILD_DIR)/crossbuilds/manylinux2010-x64/lib* $(NATIVES_DIR)/linux_64
	@mkdir -p $(NATIVES_DIR)/arm
	@cp $(ZENOHC_BUILD_DIR)/crossbuilds/linux-armv6/lib* $(NATIVES_DIR)/arm
	@mkdir -p $(NATIVES_DIR)/aarch64
	@cp $(ZENOHC_BUILD_DIR)/crossbuilds/linux-arm64/lib* $(NATIVES_DIR)/aarch64
	@mkdir -p $(NATIVES_DIR)/osx_64
	@cp $(ZENOHC_BUILD_DIR)/crossbuilds/osx-64/lib* $(NATIVES_DIR)/osx_64


$(LOCAL_LIB): $(ZENOHC_COPY_DIR)/Makefile $(SWIG_C_FILE)
	ZENOH_JAVA=ON make -C$(ZENOHC_COPY_DIR)

$(ZENOHC_COPY_DIR)/Makefile:
	@rm -fr $(ZENOHC_COPY_DIR)
	@mkdir -p $(ZENOHC_COPY_DIR)
	@if [ -z "$(ZENOH_GIT_TAG)" -a -d $(ZENOHC_DEV_DIR) ]; then \
   		echo "Found zenoh-c sources in $(ZENOHC_DEV_DIR) . Copy them to $(ZENOHC_COPY_DIR)"; \
		cp -r $(ZENOHC_DEV_DIR)/Makefile $(ZENOHC_DEV_DIR)/CMakeLists.txt $(ZENOHC_DEV_DIR)/include $(ZENOHC_DEV_DIR)/src $(ZENOHC_COPY_DIR)/ ; \
	else \
		echo "Clone tag '$${ZENOH_GIT_TAG:-master}' from Github to $(ZENOHC_COPY_DIR)" ; \
		git clone --branch $${ZENOH_GIT_TAG:-master} https://github.com/eclipse-zenoh/zenoh-c $(ZENOHC_COPY_DIR) ; \
	fi

$(SWIG_C_FILE): $(ZENOHC_COPY_DIR)/Makefile $(SWIG_FILE)
	@mkdir -p $(SWIG_JAVA_OUTDIR)
	swig -java -package org.eclipse.zenoh.swig -I$(ZENOHC_COPY_DIR)/include -outdir $(SWIG_JAVA_OUTDIR) -o $(SWIG_C_FILE) $(SWIG_FILE)


$(JNI_INCLUDE_ALL_PLATFORMS)/jni.h:
	@echo "Retrieve JNI headers for all platforms"
	@mkdir -p $(TARGET_DIR)/jni_header
	@git clone https://github.com/emabrey/jni-headers.git --depth 1 $(TARGET_DIR)/jni_header
	@mkdir -p $(JNI_INCLUDE_ALL_PLATFORMS)
	@cp -r $(TARGET_DIR)/jni_header/src/nar/resources/noarch/include/jdk/1.8.0_144-b01/* $(JNI_INCLUDE_ALL_PLATFORMS)
	@rm $(JNI_INCLUDE_ALL_PLATFORMS)/jni_md.h



clean:
	@rm -fr $(TARGET_DIR)