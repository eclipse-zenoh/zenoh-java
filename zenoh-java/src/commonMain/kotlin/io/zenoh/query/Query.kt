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

import io.zenoh.Resolvable
import io.zenoh.ZenohType
import io.zenoh.bytes.Encoding
import io.zenoh.bytes.IntoZBytes
import io.zenoh.bytes.ZBytes
import io.zenoh.exceptions.ZError
import io.zenoh.jni.JNIQuery
import io.zenoh.keyexpr.KeyExpr
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

    /**
     * Reply to the specified key expression.
     *
     * @param keyExpr Key expression to reply to. This parameter must not be necessarily the same
     * as the key expression from the Query, however it must intersect with the query key.
     * @return a [ReplySuccess.Builder]
     */
    fun reply(keyExpr: KeyExpr, payload: IntoZBytes) = ReplyBuilder(this, keyExpr, payload.into(), SampleKind.PUT)

    fun replyDel(keyExpr: KeyExpr) = ReplyBuilder(this, keyExpr, ZBytes(byteArrayOf()), SampleKind.DELETE) //TODO: refactor

    fun replyErr(payload: IntoZBytes) = ReplyErrBuilder(this, payload.into())

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

    /**
     * Perform a reply operation to the remote [Query].
     *
     * A query can not be replied more than once. After the reply is performed, the query is considered
     * to be no more valid and further attempts to reply to it will fail.
     *
     * @param reply The [Reply] to the Query.
     * @return A [Resolvable] that returns a [Result] with the status of the reply operation.
     */
    internal fun resolveReply(reply: Reply): Resolvable<Unit> = Resolvable {
        jniQuery?.apply {
            val result = when (reply) {
                is Reply.Success -> {
                    if (reply.sample.kind == SampleKind.PUT) {
                        replySuccess(reply.sample)
                    } else {
                        replyDelete(reply.sample.keyExpr, reply.sample.timestamp, reply.sample.attachment, reply.sample.qos)
                    }
                }
                is Reply.Error -> {
                    replyError(reply.error, reply.encoding)
                }
            }
            jniQuery = null
            return@Resolvable result
        }
        throw(ZError("Query is invalid"))
    }
}
