//
// Copyright (c) 2023 ZettaScale Technology
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

plugins {
    kotlin("multiplatform")
    kotlin("plugin.serialization")
    id("com.adarshr.test-logger")
    id("org.jetbrains.dokka-javadoc")
    `maven-publish`
    signing
}

val zenohFlatJniVersion: String by project
val androidEnabled = project.findProperty("android")?.toString()?.toBoolean() == true
val release = project.findProperty("release")?.toString()?.toBoolean() == true

// If the publication is meant to be done on a remote repository (Maven central).
// Modifying this property will affect the release workflows!
val isRemotePublication = project.findProperty("remotePublication")?.toString()?.toBoolean() == true

// zenoh-flat-jni is now consumed as a Maven artifact: org.eclipse.zenoh:zenoh-flat-jni:VERSION
// Native libraries are bundled in the JAR by the zenoh-flat-jni module

if (androidEnabled) {
    apply(plugin = "com.android.library")
    configureAndroid()
}


kotlin {
    jvmToolchain(11)
    jvm {
        compilations.all {
            kotlinOptions.jvmTarget = "11"
        }
        if (!androidEnabled) {
            withJava() // Adding java to a kotlin lib targeting android is incompatible
                       // The java code is only meant for testing and is non-critical for the android publication.
                       // Therefore, when enabling android we disable the java code.
        }
    }
    if (androidEnabled) {
        androidTarget {
            publishLibraryVariants("release")
        }
    }

    @Suppress("Unused")
    sourceSets {
        val commonMain by getting {
            dependencies {
                // Zenoh Flat JNI - Kotlin sources plus the native libraries.
                //
                // The two coordinates are NOT interchangeable: `zenoh-flat-jni`
                // carries the six desktop targets, `zenoh-flat-jni-android` the
                // four Android ABIs as `jni/<abi>/`. An Android build that
                // depended on the desktop artifact would ship without any
                // loadable library.
                //
                // They are selected here, rather than per source set, because
                // commonMain references the generated classes and Kotlin
                // Multiplatform cannot see a dependency declared only in the
                // platform source sets. `-Pandroid=true` therefore has to select
                // the whole build's flavour — which is also why the JVM and
                // Android publications must be produced by separate Gradle
                // invocations. See PUBLISHING.md.
                //
                // Version lives in gradle.properties so the release can bump it
                // and a rehearsal can point at a snapshot.
                val flatJniArtifact =
                    if (androidEnabled) "zenoh-flat-jni-android" else "zenoh-flat-jni"
                implementation("org.eclipse.zenoh:$flatJniArtifact:$zenohFlatJniVersion")
                implementation("com.google.guava:guava:33.3.1-jre")
            }
        }
        val commonTest by getting {
            dependencies {
                implementation(kotlin("test"))
                // Only the tests need an NTP64 clock now: the SDK's Timestamp
                // carries the raw bits and an originating-node id, so it no
                // longer depends on commons-net itself.
                implementation("commons-net:commons-net:3.9.0")
            }
        }
        if (androidEnabled) {
            val androidUnitTest by getting {
                dependencies {
                    implementation(kotlin("test-junit"))
                }
            }
        }
    }

    val javadocJar by tasks.registering(Jar::class) {
        dependsOn("dokkaGeneratePublicationJavadoc")
        archiveClassifier.set("javadoc")
        // Dokka's javadoc publication writes here. It used to say `dokka/html`,
        // which Dokka never produces, so the published javadoc JAR contained
        // nothing but a manifest.
        val javadocDir = layout.buildDirectory.dir("dokka/javadoc")
        from(javadocDir)
        doFirst {
            val dir = javadocDir.get().asFile
            check(dir.isDirectory && (dir.list()?.isNotEmpty() == true)) {
                "$dir is missing or empty — the javadoc JAR would ship empty"
            }
        }
    }

    publishing {
        publications.withType<MavenPublication> {
            groupId = "org.eclipse.zenoh"
            artifactId = "zenoh-java"
            version = rootProject.version.toString()

            artifact(javadocJar)

            pom {
                name.set("Zenoh Java")
                description.set("The Eclipse Zenoh: Zero Overhead Pub/sub, Store/Query and Compute.")
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
                    developer {
                        id.set("DariusIMP")
                        name.set("Darius Maitia")
                        email.set("darius@zettascale.tech")
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

fun Project.configureAndroid() {
    extensions.configure<com.android.build.gradle.LibraryExtension>("android") {
        namespace = "io.zenoh"
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
