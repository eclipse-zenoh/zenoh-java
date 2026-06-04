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
import io.zenoh.jni.query.ZQuery
import io.zenoh.keyexpr.KeyExpr

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
 * @property acceptsReplies The [ReplyKeyExpr] indicating what key expressions are accepted in replies.
 */
class Query internal constructor(
    val keyExpr: KeyExpr,
    val selector: Selector,
    val payload: ZBytes?,
    val encoding: Encoding?,
    val attachment: ZBytes?,
    val acceptsReplies: ReplyKeyExpr,
    private var zQuery: ZQuery?
) : AutoCloseable, ZenohType {

    /** Shortcut to the [selector]'s parameters. */
    val parameters = selector.parameters

    internal companion object {
        /** Repacks the flat [io.zenoh.jni.query.Query] data class delivered by a queryable callback. */
        fun from(flat: io.zenoh.jni.query.Query): Query {
            val ke = KeyExpr(flat.keyExpr)
            val selector = if (flat.parameters.isEmpty()) Selector(ke)
                           else Selector(ke, Parameters.from(flat.parameters))
            return Query(
                ke,
                selector,
                flat.payload?.let { ZBytes(it) },
                flat.encoding?.let { Encoding(it) },
                flat.attachment?.let { ZBytes(it) },
                flat.acceptsReplies.toPublic(),
                flat.query
            )
        }

        /** Builds a [Query] from the flattened leaf wires the JNI callback delivers. */
        fun fromFlat(
            keyExprString: String,
            keyExprNative: Long,
            parameters: String,
            payload: ByteArray?,
            encodingPresent: Boolean,
            encodingId: Int,
            encodingSchema: String?,
            attachment: ByteArray?,
            acceptsReplies: Int,
            query: Long,
        ): Query = from(
            io.zenoh.jni.query.Query.fromParts(
                keyExprString, keyExprNative, parameters, payload, encodingPresent, encodingId,
                encodingSchema, attachment, acceptsReplies, query
            )
        )
    }

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
        val q = zQuery ?: throw ZError("Query is invalid")
        q.queryReplySuccess(
            keyExpr.flat,
            payload.into().inner,
            options.encoding.toFlat(),
            options.timeStamp?.ntpValue(),
            options.attachment?.into()?.inner,
            options.express
        )
        zQuery = null
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
        val q = zQuery ?: throw ZError("Query is invalid")
        q.queryReplyDelete(
            keyExpr.flat,
            options.timeStamp?.ntpValue(),
            options.attachment?.into()?.inner,
            options.express
        )
        zQuery = null
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
        val q = zQuery ?: throw ZError("Query is invalid")
        q.queryReplyError(message.into().inner, options.encoding.toFlat())
        zQuery = null
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
        zQuery?.close()
        zQuery = null
    }

    @Suppress("removal")
    protected fun finalize() {
        close()
    }
}
