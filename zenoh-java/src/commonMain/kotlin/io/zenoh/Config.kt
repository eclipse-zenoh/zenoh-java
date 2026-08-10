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

import io.zenoh.exceptions.ZError
import io.zenoh.exceptions.throwZError
import io.zenoh.exceptions.throwZError0
import io.zenoh.jni.config.Config as JniConfig
import java.io.File
import java.nio.file.Path

/**
 * # Config
 *
 * Config class to set the Zenoh configuration to be used through a [io.zenoh.Session].
 *
 * The configuration can be specified in two different ways:
 * - By providing a file or a path to a file with the configuration
 * - By providing a raw string configuration.
 *
 * Either way, the supported formats are `yaml`, `json` and `json5`.
 *
 * A default configuration can be loaded using [Config.loadDefault].
 *
 * [Zenoh.open] (and [Zenoh.scout]) copy the native configuration rather than taking it, so a Config stays
 * usable afterwards and its native memory is released by [close] — or, if never closed, by the
 * garbage-collection backstop.
 *
 * Visit the [default configuration](https://github.com/eclipse-zenoh/zenoh/blob/main/DEFAULT_CONFIG.json5) for more
 * information on the Zenoh config parameters.
 */
class Config internal constructor(internal val zConfig: JniConfig) : AutoCloseable {

    companion object {

        private const val CONFIG_ENV = "ZENOH_CONFIG"

        /**
         * Returns the default config.
         */
        @JvmStatic
        fun loadDefault(): Config = Config(JniConfig.newDefault(throwZError0))

        /**
         * Loads the configuration from the [File] specified.
         *
         * @param file The Zenoh config file. Supported types are: JSON, JSON5 and YAML.
         *   Note the format is determined after the file extension.
         * @return The [Config].
         */
        @JvmStatic
        @Throws(ZError::class)
        fun fromFile(file: File): Config = fromFile(file.toPath())

        /**
         * Loads the configuration from the [Path] specified.
         *
         * @param path Path to the Zenoh config file. Supported types are: JSON, JSON5 and YAML.
         *   Note the format is determined after the file extension.
         * @return The [Config].
         */
        @JvmStatic
        @Throws(ZError::class)
        fun fromFile(path: Path): Config = Config(JniConfig.newFromFile(path.toString(), throwZError0, throwZError))

        /**
         * Loads the configuration from json-formatted string.
         *
         * Visit the [default configuration](https://github.com/eclipse-zenoh/zenoh/blob/main/DEFAULT_CONFIG.json5) for more
         * information on the Zenoh config parameters.
         *
         * @param config Json formatted config.
         * @return The [Config].
         */
        @JvmStatic
        @Throws(ZError::class)
        // Parsed by the JSON5 reader: base zenoh's `Config` has no `from_json`,
        // and JSON is a subset of JSON5, so every input accepted here before is
        // still accepted and parses to the same config.
        fun fromJson(config: String): Config = Config(JniConfig.newFromJson5(config, throwZError0, throwZError))

        /**
         * Loads the configuration from json5-formatted string.
         *
         * Visit the [default configuration](https://github.com/eclipse-zenoh/zenoh/blob/main/DEFAULT_CONFIG.json5) for more
         * information on the Zenoh config parameters.
         *
         * @param config Json5 formatted config
         * @return The [Config].
         */
        @JvmStatic
        @Throws(ZError::class)
        fun fromJson5(config: String): Config = Config(JniConfig.newFromJson5(config, throwZError0, throwZError))

        /**
         * Loads the configuration from yaml-formatted string.
         *
         * Visit the [default configuration](https://github.com/eclipse-zenoh/zenoh/blob/main/DEFAULT_CONFIG.json5) for more
         * information on the Zenoh config parameters.
         *
         * @param config Yaml formatted config
         * @return The [Config].
         */
        @JvmStatic
        @Throws(ZError::class)
        fun fromYaml(config: String): Config = Config(JniConfig.newFromYaml(config, throwZError0, throwZError))

        /**
         * Loads the configuration from the env variable [CONFIG_ENV].
         *
         * @return The config.
         */
        @JvmStatic
        @Throws(ZError::class)
        fun fromEnv(): Config {
            val envValue = System.getenv(CONFIG_ENV)
            if (envValue != null) {
                return fromFile(File(envValue))
            } else {
                throw Exception("Couldn't load env variable: $CONFIG_ENV.")
            }
        }
    }

    /**
     * The json value associated to the [key].
     */
    @Throws(ZError::class)
    fun getJson(key: String): String = zConfig.getJson(key, throwZError0, throwZError)

    /**
     * Inserts a json5 value associated to the [key] into the Config.
     */
    @Throws(ZError::class)
    fun insertJson5(key: String, value: String) =
        zConfig.insertJson5(key, value, throwZError0, throwZError)

    /**
     * Releases the native configuration. Idempotent. Any later use of this
     * Config — [getJson], [insertJson5], or opening a session with it — fails
     * with a [ZError] reporting a closed handle.
     */
    override fun close() = zConfig.close()
}
