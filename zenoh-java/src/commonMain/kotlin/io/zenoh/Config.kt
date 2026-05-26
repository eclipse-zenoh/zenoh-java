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

import io.zenoh.ZenohLoad
import io.zenoh.exceptions.ZError
import io.zenoh.exceptions.wrapJNIExceptionAsZError
import io.zenoh.jni.ZConfig
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
 * Visit the [default configuration](https://github.com/eclipse-zenoh/zenoh/blob/main/DEFAULT_CONFIG.json5) for more
 * information on the Zenoh config parameters.
 */
class Config internal constructor(internal val zConfig: ZConfig) {

    companion object {

        init {
            ZenohLoad
        }

        private const val CONFIG_ENV = "ZENOH_CONFIG"

        /**
         * Returns the default config.
         */
        @JvmStatic
        fun loadDefault(): Config = wrapJNIExceptionAsZError {
            Config(ZConfig.zConfigDefault())
        }

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
        fun fromFile(path: Path): Config = wrapJNIExceptionAsZError {
            Config(ZConfig.zConfigFromFile(path.toString()))
        }

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
        fun fromJson(config: String): Config = wrapJNIExceptionAsZError {
            Config(ZConfig.zConfigFromJson(config))
        }

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
        fun fromJson5(config: String): Config = wrapJNIExceptionAsZError {
            Config(ZConfig.zConfigFromJson5(config))
        }

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
        fun fromYaml(config: String): Config = wrapJNIExceptionAsZError {
            Config(ZConfig.zConfigFromYaml(config))
        }

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
    fun getJson(key: String): String = wrapJNIExceptionAsZError {
        zConfig.zConfigGetJson(key)
    }

    /**
     * Inserts a json5 value associated to the [key] into the Config.
     */
    @Throws(ZError::class)
    fun insertJson5(key: String, value: String) = wrapJNIExceptionAsZError {
        zConfig.zConfigInsertJson5(key, value)
    }
}
