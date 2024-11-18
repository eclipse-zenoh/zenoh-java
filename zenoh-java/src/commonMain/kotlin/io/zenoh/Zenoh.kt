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

import io.zenoh.Logger.Companion.LOG_ENV
import io.zenoh.scouting.Hello
import io.zenoh.scouting.Scout
import io.zenoh.exceptions.ZError
import io.zenoh.handlers.BlockingQueueHandler
import io.zenoh.scouting.ScoutBuilder
import java.util.*
import java.util.concurrent.BlockingQueue
import java.util.concurrent.LinkedBlockingDeque

object Zenoh {

    /**
     * Open a [Session] with the provided [Config].
     *
     * @param config The configuration for the session.
     * @return The [Session] on success.
     */
    @JvmStatic
    @Throws(ZError::class)
    fun open(config: Config): Session {
        return Session.open(config)
    }

    /**
     * Scout for routers and/or peers.
     *
     * Scout spawns a task that periodically sends scout messages and waits for Hello replies.
     * Drop the returned Scout to stop the scouting task or explicitly call [Scout.stop] or [Scout.close].
     */
    @JvmStatic
    fun scoutBuilder(): ScoutBuilder<BlockingQueue<Optional<Hello>>> {
        return ScoutBuilder(handler = BlockingQueueHandler(queue = LinkedBlockingDeque()))
    }

    /**
     * Initializes the zenoh runtime logger, using rust environment settings.
     * E.g.: `RUST_LOG=info` will enable logging at info level. Similarly, you can set the variable to `error` or `debug`.
     *
     * Note that if the environment variable is not set, then logging will not be enabled.
     * See https://docs.rs/env_logger/latest/env_logger/index.html for accepted filter format.
     *
     * @see Logger
     */
    @JvmStatic
    fun tryInitLogFromEnv() {
        val logEnv = System.getenv(LOG_ENV)
        if (logEnv != null) {
            ZenohLoad
            Logger.start(logEnv)
        }
    }

    /**
     * Initializes the zenoh runtime logger, using rust environment settings or the provided fallback level.
     * E.g.: `RUST_LOG=info` will enable logging at info level. Similarly, you can set the variable to `error` or `debug`.
     *
     * Note that if the environment variable is not set, then [fallbackFilter] will be used instead.
     * See https://docs.rs/env_logger/latest/env_logger/index.html for accepted filter format.
     *
     * @param fallbackFilter: The fallback filter if the `RUST_LOG` environment variable is not set.
     * @see Logger
     */
    @JvmStatic
    fun initLogFromEnvOr(fallbackFilter: String): Result<Unit> = runCatching {
        ZenohLoad
        val logLevelProp = System.getenv(LOG_ENV)
        logLevelProp?.let { Logger.start(it) } ?: Logger.start(fallbackFilter)
    }
}

/**
 * Static singleton class to load the Zenoh native library once and only once, as well as the logger in function of the
 * log level configuration.
 */
internal expect object ZenohLoad
