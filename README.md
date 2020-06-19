![zenoh banner](./zenoh-dragon.png)

![Build](https://github.com/eclipse-zenoh/zenoh-java/workflows/Build%20(for%20all%20supported%20platform)/badge.svg)
[![Documentation Status](https://readthedocs.org/projects/zenoh-java/badge/?version=latest)](https://zenoh-java.readthedocs.io/en/latest/?badge=latest)
[![Gitter](https://badges.gitter.im/atolab/zenoh.svg)](https://gitter.im/atolab/zenoh?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)
[![License](https://img.shields.io/badge/License-EPL%202.0-blue)](https://choosealicense.com/licenses/epl-2.0/)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

# Eclipse zenoh Java Client API

The Java API for [Eclipse zenoh](https://zenoh.io), based on the zenoh-c API via JNI.

## Installation

zenoh-java is available on Maven Central.
Just add the dependency in your POM:
```xml
  <dependency>
    <groupId>org.eclipse.zenoh</groupId>
    <artifactId>zenoh</artifactId>
    <version>0.4.2-M1</version>
  </dependency>
```

## Building
Requirements:

 - [Java >= 8](http://openjdk.java.net)
 - [Apache Maven >= 3.6.0](https://maven.apache.org/download.cgi)
 - [SWIG](http://swig.org)
 - [CMake](https://cmake.org)
 - make, gcc (for zenoh-c compilation)

Optional for cross-compilation:
 - Docker

To build for your current platform:
```mvn clean install```

If zenoh-c is found in the same directory than zenoh-java, the build will copy its sources and compile it.
Otherwise, the build will clone the [zenoh-c](https://github.com/eclipse-zenoh/zenoh-c) repository and compile it.

Note that this Maven build offers profiles in addition of the default one:

 - ```mvn -Pdebug clean install```

    - compiles zenoh-c with debug logs active

 - ```mvn -Prelease clean install```

   - compiles zenoh-c in release mode (without logs)
   - cross-compiles zenoh-c on all supported platforms (incl. MacOS if this is your current host) using [dockross](https://github.com/dockcross/dockcross).
   - generates the Javadoc
   - generate a ZIP file for release in assembly/target


## Examples
See [examples/zenoh/README.md](examples/zenoh/)
