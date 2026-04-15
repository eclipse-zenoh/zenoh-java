//
// Copyright (c) 2026 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//

import com.nishtahir.CargoExtension

plugins {
    kotlin("multiplatform")
    `maven-publish`
    signing
}

val androidEnabled = project.findProperty("android")?.toString()?.toBoolean() == true
val release = project.findProperty("release")?.toString()?.toBoolean() == true

// If the publication is meant to be done on a remote repository (Maven central).
// Modifying this property will affect the release workflows!
val isRemotePublication = project.findProperty("remotePublication")?.toString()?.toBoolean() == true

var buildMode = if (release) BuildMode.RELEASE else BuildMode.DEBUG

if (androidEnabled) {
    apply(plugin = "com.android.library")
    apply(plugin = "org.mozilla.rust-android-gradle.rust-android")

    configureCargo()
    configureAndroid()
}

kotlin {
    jvmToolchain(11)
    jvm {
        compilations.all {
            kotlinOptions.jvmTarget = "11"
        }
        testRuns["test"].executionTask.configure {
            val zenohPaths = "../zenoh-jni/target/$buildMode"
            jvmArgs("-Djava.library.path=$zenohPaths")
        }
        if (!androidEnabled) {
            withJava()
        }
    }
    if (androidEnabled) {
        androidTarget {
            publishLibraryVariants("release")
        }
    }

    @Suppress("Unused")
    sourceSets {
        val commonMain by getting {}
        val commonTest by getting {
            dependencies {
                implementation(kotlin("test"))
            }
        }
        if (androidEnabled) {
            val androidUnitTest by getting {
                dependencies {
                    implementation(kotlin("test-junit"))
                }
            }
        }
        val jvmMain by getting {
            if (isRemotePublication) {
                resources.srcDir("../jni-libs").include("*/**")
            } else {
                resources.srcDir("../zenoh-jni/target/$buildMode").include(arrayListOf("*.dylib", "*.so", "*.dll"))
            }
        }

        val jvmTest by getting {
            resources.srcDir("../zenoh-jni/target/$buildMode").include(arrayListOf("*.dylib", "*.so", "*.dll"))
        }
    }

    publishing {
        publications.withType<MavenPublication> {
            groupId = "org.eclipse.zenoh"
            artifactId = "zenoh-jni-runtime"
            version = rootProject.version.toString()

            pom {
                name.set("Zenoh JNI Runtime")
                description.set("The Eclipse Zenoh JNI runtime layer for zenoh-java and zenoh-kotlin.")
                url.set("https://zenoh.io/")

                licenses {
                    license {
                        name.set("Eclipse Public License 2.0 OR Apache License 2.0")
                        url.set("http://www.eclipse.org/legal/epl-2.0")
                    }
                }
                developers {
                    developer {
                        id.set("ZettaScale")
                        name.set("ZettaScale Zenoh Team")
                        email.set("zenoh@zettascale.tech")
                    }
                }
                scm {
                    connection.set("scm:git:https://github.com/eclipse-zenoh/zenoh-java.git")
                    developerConnection.set("scm:git:https://github.com/eclipse-zenoh/zenoh-java.git")
                    url.set("https://github.com/eclipse-zenoh/zenoh-java")
                }
            }
        }
    }
}

signing {
    isRequired = isRemotePublication
    useInMemoryPgpKeys(System.getenv("ORG_GPG_SUBKEY_ID"), System.getenv("ORG_GPG_PRIVATE_KEY"), System.getenv("ORG_GPG_PASSPHRASE"))
    sign(publishing.publications)
}

tasks.withType<PublishToMavenRepository>().configureEach {
    dependsOn(tasks.withType<Sign>())
}

tasks.withType<Test> {
    doFirst {
        systemProperty("java.library.path", "../zenoh-jni/target/$buildMode")
    }
}

tasks.whenObjectAdded {
    if ((this.name == "mergeDebugJniLibFolders" || this.name == "mergeReleaseJniLibFolders")) {
        this.dependsOn("cargoBuild")
    }
}

tasks.named("compileKotlinJvm") {
    dependsOn("buildZenohJni")
}

tasks.register("buildZenohJni") {
    doLast {
        if (!isRemotePublication) {
            buildZenohJNI(buildMode)
        }
    }
}

fun buildZenohJNI(mode: BuildMode = BuildMode.DEBUG) {
    val cargoCommand = mutableListOf("cargo", "build")

    if (mode == BuildMode.RELEASE) {
        cargoCommand.add("--release")
    }

    val result = project.exec {
        commandLine(*(cargoCommand.toTypedArray()), "--manifest-path", "../zenoh-jni/Cargo.toml")
    }

    if (result.exitValue != 0) {
        throw GradleException("Failed to build Zenoh-JNI.")
    }

    Thread.sleep(1000)
}

enum class BuildMode {
    DEBUG {
        override fun toString(): String {
            return "debug"
        }
    },
    RELEASE {
        override fun toString(): String {
            return "release"
        }
    }
}

fun Project.configureAndroid() {
    extensions.configure<com.android.build.gradle.LibraryExtension>("android") {
        namespace = "io.zenoh.jni"
        compileSdk = 30

        ndkVersion = "26.0.10792818"

        defaultConfig {
            minSdk = 30
        }

        compileOptions {
            sourceCompatibility = JavaVersion.VERSION_11
            targetCompatibility = JavaVersion.VERSION_11
        }

        buildTypes {
            getByName("release") {
                isMinifyEnabled = false
            }
            getByName("debug") {
                isMinifyEnabled = false
            }
        }
        sourceSets {
            getByName("main") {
                manifest.srcFile("src/androidMain/AndroidManifest.xml")
            }
        }
        publishing {
            singleVariant("release") {
                withSourcesJar()
                withJavadocJar()
            }
        }
    }
}

fun Project.configureCargo() {
    extensions.configure<CargoExtension>("cargo") {
        pythonCommand = "python3"
        module = "../zenoh-jni"
        libname = "zenoh-jni"
        targetIncludes = arrayOf("libzenoh_jni.so")
        targetDirectory = "../zenoh-jni/target/"
        profile = "release"
        targets = arrayListOf(
            "arm",
            "arm64",
            "x86",
            "x86_64",
        )
    }
}
