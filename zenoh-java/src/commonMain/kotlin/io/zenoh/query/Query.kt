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

package io.zenoh.query

import io.zenoh.ZenohType
import io.zenoh.bytes.Encoding
import io.zenoh.bytes.IntoZBytes
import io.zenoh.bytes.ZBytes
import io.zenoh.exceptions.ZError
import io.zenoh.jni.JNIQuery
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.qos.QoS
import io.zenoh.sample.Sample
import io.zenoh.sample.SampleKind

/**
 * Represents a Zenoh Query in Kotlin.
 *
 * A Query is generated within the context of a [Queryable], when receiving a [Query] request.
 *
 * @property keyExpr The key expression to which the query is associated.
 * @property selector The selector
 * @property payload Optional payload in case the received query was declared using "with query".
 * @property encoding Encoding of the [payload].
 * @property attachment Optional attachment.
 */
class Query internal constructor(
    val keyExpr: KeyExpr,
    val selector: Selector,
    val payload: ZBytes?,
    val encoding: Encoding?,
    val attachment: ZBytes?,
    private var jniQuery: JNIQuery?
) : AutoCloseable, ZenohType {

    /** Shortcut to the [selector]'s parameters. */
    val parameters = selector.parameters

    /**
     * Reply to the specified key expression.
     *
     * @param keyExpr Key expression to reply to. This parameter must not be necessarily the same
     * as the key expression from the Query, however it must intersect with the query key.
     * @param payload The reply payload.
     * @param options Optional options for configuring the reply.
     */
    @Throws(ZError::class)
    @JvmOverloads
    fun reply(keyExpr: KeyExpr, payload: IntoZBytes, options: ReplyOptions = ReplyOptions()) {
        val sample = Sample(
            keyExpr,
            payload.into(),
            options.encoding,
            SampleKind.PUT,
            options.timeStamp,
            QoS(options.congestionControl, options.priority, options.express),
            options.attachment?.into()
        )
        jniQuery?.apply {
            replySuccess(sample)
            jniQuery = null
        } ?: throw (ZError("Query is invalid"))
    }

    /**
     * Reply to the specified key expression.
     *
     * @param keyExpr Key expression to reply to. This parameter must not be necessarily the same
     * as the key expression from the Query, however it must intersect with the query key.
     * @param payload The reply payload as a string.
     * @param options Optional options for configuring the reply.
     */
    @Throws(ZError::class)
    @JvmOverloads
    fun reply(keyExpr: KeyExpr, payload: String, options: ReplyOptions = ReplyOptions()) = reply(keyExpr, ZBytes.from(payload), options)

    /**
     * Reply "delete" to the specified key expression.
     *
     * @param keyExpr Key expression to reply to. This parameter must not be necessarily the same
     * as the key expression from the Query, however it must intersect with the query key.
     * @param options Optional options for configuring the reply.
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun replyDel(keyExpr: KeyExpr, options: ReplyDelOptions = ReplyDelOptions()) {
        jniQuery?.apply {
            replyDelete(
                keyExpr,
                options.timeStamp,
                options.attachment,
                QoS(options.congestionControl, options.priority, options.express),
            )
            jniQuery = null
        } ?: throw (ZError("Query is invalid"))
    }

    /**
     * Reply "error" to the specified key expression.
     *
     * @param message The error message.
     * @param options Optional options for configuring the reply.
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun replyErr(message: IntoZBytes, options: ReplyErrOptions = ReplyErrOptions()) {
        jniQuery?.apply {
            replyError(message.into(), options.encoding)
            jniQuery = null
        } ?: throw (ZError("Query is invalid"))
    }

    /**
     * Reply "error" to the specified key expression.
     *
     * @param message The error message as a String.
     * @param options Optional options for configuring the reply.
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun replyErr(message: String, options: ReplyErrOptions = ReplyErrOptions()) = replyErr(ZBytes.from(message), options)

    override fun close() {
        jniQuery?.apply {
            this.close()
            jniQuery = null
        }
    }

    @Suppress("removal")
    protected fun finalize() {
        close()
    }
}
