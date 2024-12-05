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
 * @property jniQuery Delegate object in charge of communicating with the underlying native code.
 * @constructor Instances of Query objects are only meant to be created through the JNI upon receiving
 * a query request. Therefore, the constructor is private.
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

    @Throws(ZError::class)
    fun reply(keyExpr: KeyExpr, payload: IntoZBytes) = reply(keyExpr, payload, ReplyOptions())

    /**
     * Reply to the specified key expression.
     *
     * @param keyExpr Key expression to reply to. This parameter must not be necessarily the same
     * as the key expression from the Query, however it must intersect with the query key.
     */
    @Throws(ZError::class)
    fun reply(keyExpr: KeyExpr, payload: IntoZBytes, config: ReplyOptions) {
        val sample = Sample(
            keyExpr,
            payload.into(),
            config.encoding,
            SampleKind.PUT,
            config.timeStamp,
            config.qos,
            config.attachment
        )
        jniQuery?.apply {
            replySuccess(sample)
            jniQuery = null
        } ?: throw (ZError("Query is invalid"))
    }

    /**
     * TODO
     */
    @Throws(ZError::class)
    fun replyDel(keyExpr: KeyExpr) = replyDel(keyExpr, ReplyDelConfig())

    /**
     * TODO
     */
    @Throws(ZError::class)
    fun replyDel(keyExpr: KeyExpr, config: ReplyDelConfig) {
        jniQuery?.apply {
            replyDelete(
                keyExpr,
                config.timeStamp,
                config.attachment,
                config.qos
            )
            jniQuery = null
        } ?: throw (ZError("Query is invalid"))
    }

    /**
     * TODO
     */
    @Throws(ZError::class)
    fun replyErr(payload: IntoZBytes) = replyErr(payload, ReplyErrConfig())

    /**
     * TODO
     */
    @Throws(ZError::class)
    fun replyErr(payload: IntoZBytes, config: ReplyErrConfig) {
        jniQuery?.apply {
            replyError(payload.into(), config.encoding)
            jniQuery = null
        } ?: throw (ZError("Query is invalid"))
    }

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
