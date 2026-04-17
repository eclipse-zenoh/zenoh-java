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

package io.zenoh

import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.InputStream
import java.util.zip.ZipInputStream

/**
 * Static singleton class to load the Zenoh native library once and only once, as well as the logger in function of the
 * log level configuration.
 */
public actual object ZenohLoad {
    private const val ZENOH_LIB_NAME = "zenoh_jni"

    init {
        // Try first to load the local native library for cases in which the module was built locally,
        // otherwise try to load from the JAR.
        if (tryLoadingLocalLibrary().isFailure) {
            val target = determineTarget().getOrThrow()
            tryLoadingLibraryFromJarPackage(target).getOrThrow()
        }
    }

    private fun determineTarget(): Result<Target> = runCatching {
        val osName = System.getProperty("os.name").lowercase()
        val osArch = System.getProperty("os.arch").lowercase()

        val target = when {
            osName.contains("win") -> when {
                osArch.contains("x86_64") || osArch.contains("amd64") || osArch.contains("x64") ->
                    Target.WINDOWS_X86_64_MSVC

                osArch.contains("aarch64") || osArch.contains("arm64") ->
                    Target.WINDOWS_AARCH64_MSVC

                else -> throw UnsupportedOperationException("Unsupported architecture on Windows: $osArch")
            }

            osName.contains("mac") || osName.contains("darwin") || osName.contains("os x") -> when {
                osArch.contains("x86_64") || osArch.contains("amd64") || osArch.contains("x64") ->
                    Target.APPLE_X86_64

                osArch.contains("aarch64") || osArch.contains("arm64") ->
                    Target.APPLE_AARCH64

                else -> throw UnsupportedOperationException("Unsupported architecture on macOS: $osArch")
            }

            osName.contains("nix") || osName.contains("nux") || osName.contains("aix") -> when {
                osArch.contains("x86_64") || osArch.contains("amd64") || osArch.contains("x64") ->
                    Target.LINUX_X86_64

                osArch.contains("aarch64") || osArch.contains("arm64") ->
                    Target.LINUX_AARCH64

                else -> throw UnsupportedOperationException("Unsupported architecture on Linux/Unix: $osArch")
            }

            else -> throw UnsupportedOperationException("Unsupported platform: $osName")
        }
        return Result.success(target)
    }

    private fun unzipLibrary(compressedLib: InputStream): Result<File> = runCatching {
        val zipInputStream = ZipInputStream(compressedLib)
        val buffer = ByteArray(1024)
        val zipEntry = zipInputStream.nextEntry

        val library = File.createTempFile(zipEntry!!.name, ".tmp")
        library.deleteOnExit()

        val parent = library.parentFile
        if (!parent.exists()) {
            parent.mkdirs()
        }

        val fileOutputStream = FileOutputStream(library)
        var len: Int
        while (zipInputStream.read(buffer).also { len = it } > 0) {
            fileOutputStream.write(buffer, 0, len)
        }
        fileOutputStream.close()

        zipInputStream.closeEntry()
        zipInputStream.close()
        return Result.success(library)
    }

    private fun loadLibraryAsInputStream(target: Target): Result<InputStream> = runCatching {
        val targetName = "$target/$target.zip"
        val libUrl = ClassLoader.getSystemClassLoader().getResourceAsStream(targetName)
            ?: javaClass.classLoader.getResourceAsStream(targetName)!!
        val uncompressedLibFile = unzipLibrary(libUrl)
        return Result.success(FileInputStream(uncompressedLibFile.getOrThrow()))
    }

    @Suppress("UnsafeDynamicallyLoadedCode")
    private fun loadZenohJNI(inputStream: InputStream) {
        val tempLib = File.createTempFile("tempLib", ".tmp")
        tempLib.deleteOnExit()

        FileOutputStream(tempLib).use { output ->
            inputStream.copyTo(output)
        }

        System.load(tempLib.absolutePath)
    }

    private fun tryLoadingLibraryFromJarPackage(target: Target): Result<Unit> = runCatching {
        val lib: Result<InputStream> = loadLibraryAsInputStream(target)
        lib.onSuccess { loadZenohJNI(it) }.onFailure { throw Exception("Unable to load Zenoh JNI: $it") }
    }

    private fun tryLoadingLocalLibrary(): Result<Unit> = runCatching {
        val lib = ClassLoader.getSystemClassLoader().findLibraryStream(ZENOH_LIB_NAME)
            ?: javaClass.classLoader.findLibraryStream(
                ZENOH_LIB_NAME
            )
        if (lib != null) {
            loadZenohJNI(lib)
        } else {
            throw Exception("Unable to load local Zenoh JNI.")
        }
    }
}

private fun ClassLoader.findLibraryStream(libraryName: String): InputStream? {
    val libraryExtensions = listOf(".dylib", ".so", ".dll")
    for (extension in libraryExtensions) {
        val resourcePath = "lib$libraryName$extension"
        val inputStream = getResourceAsStream(resourcePath)
        if (inputStream != null) {
            return inputStream
        }
    }
    return null
}
