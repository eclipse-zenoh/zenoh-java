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
import io.zenoh.bytes.forWire
import io.zenoh.bytes.IntoZBytes
import io.zenoh.bytes.ZBytes
import io.zenoh.exceptions.ZError
import io.zenoh.exceptions.throwZError
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
    private var zQuery: io.zenoh.jni.query.Query?
) : AutoCloseable, ZenohType {

    /** Shortcut to the [selector]'s parameters. */
    val parameters = selector.parameters

    internal companion object {
        /**
         * Builds an SDK [Query] from a queryable callback's natively-decomposed
         * leaves (delivered in ONE JNI crossing as raw handles + primitives —
         * heavy data is read lazily on demand). `zq` is the owned query
         * handle, **retained** because the reply methods consume it (replying
         * keeps working after the callback returns).
         */
        fun fromParts(
            keH: io.zenoh.jni.keyexpr.KeyExpr,
            parameters: String,
            payloadH: io.zenoh.jni.bytes.ZBytes?,
            encId: Int?,
            encSchema: String?,
            attachH: io.zenoh.jni.bytes.ZBytes?,
            acceptsRepliesInt: Int,
            zq: io.zenoh.jni.query.Query,
        ): Query {
            val ke = KeyExpr(keH)
            val selector = if (parameters.isEmpty()) Selector(ke)
                           else Selector(ke, Parameters.from(parameters))
            return Query(
                ke,
                selector,
                payloadH?.let { ZBytes.fromHandle(it) },
                encId?.let { Encoding(it, encSchema) },
                attachH?.let { ZBytes.fromHandle(it) },
                io.zenoh.jni.query.ReplyKeyExpr.fromInt(acceptsRepliesInt).toPublic(),
                zq
            )
        }
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
        val enc = options.encoding.forWire()
        q.replySuccess(
            keyExpr.flat,
            payload.into().bytes,
            enc.sel,
            enc.id,
            enc.schema,
            enc.handle,
            options.timeStamp?.ntpValue(),
            options.attachment?.into()?.bytes,
            options.express,
            throwZError
        )
        // Single-reply model: dropping the native query finalizes the reply
        // stream so the querier's get completes. Safe whether the query came
        // straight from the callback or was carried across a channel.
        q.close()
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
        q.replyDelete(
            keyExpr.flat,
            options.timeStamp?.ntpValue(),
            options.attachment?.into()?.bytes,
            options.express,
            throwZError
        )
        q.close()
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
        val enc = options.encoding.forWire()
        q.replyError(message.into().bytes, enc.sel, enc.id, enc.schema, enc.handle, throwZError)
        q.close()
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
