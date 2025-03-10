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

package io.zenoh;

import io.zenoh.bytes.Encoding;
import io.zenoh.bytes.ZBytes;
import io.zenoh.exceptions.ZError;
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.qos.CongestionControl;
import io.zenoh.qos.Priority;
import io.zenoh.qos.QoS;
import io.zenoh.query.Querier;
import io.zenoh.query.Reply;
import io.zenoh.query.ReplyOptions;
import io.zenoh.sample.Sample;
import io.zenoh.sample.SampleKind;
import org.apache.commons.net.ntp.TimeStamp;
import org.junit.Test;

import java.time.Instant;
import java.util.*;

import static org.junit.Assert.*;

public class QuerierTest {

    static final ZBytes testPayload = ZBytes.from("Hello queryable");
    static final KeyExpr testKeyExpr;

    static {
        try {
            testKeyExpr = KeyExpr.tryFrom("example/testing/keyexpr");
        } catch (ZError e) {
            throw new RuntimeException(e);
        }
    }

    /**
     * Test validating both Queryable and get operations.
     */
    @Test
    public void querier_runsWithCallbackTest() throws ZError {
        var sample = new Sample(
                testKeyExpr,
                testPayload,
                Encoding.defaultEncoding(),
                SampleKind.PUT,
                new TimeStamp(Date.from(Instant.now())),
                new QoS(CongestionControl.BLOCK, Priority.DATA, false),
                null
        );
        var examplePayload = ZBytes.from("Example payload");
        var exampleAttachment = ZBytes.from("Example attachment");
        var session = Zenoh.open(Config.loadDefault());

        var queryable = session.declareQueryable(testKeyExpr, query -> {
                assertEquals(exampleAttachment, query.getAttachment());
                assertEquals(examplePayload, query.getPayload());

                var replyOptions = new ReplyOptions();
                replyOptions.setTimeStamp(sample.getTimestamp());
            try {
                query.reply(testKeyExpr, sample.getPayload(), replyOptions);
            } catch (ZError e) {
                throw new RuntimeException(e);
            }
        });

        var querier = session.declareQuerier(testKeyExpr);

        Reply[] receivedReply = new Reply[1];
        var options = new Querier.GetOptions();
        options.setPayload(examplePayload);
        options.setAttachment(exampleAttachment);
        querier.get(
            reply -> {
                receivedReply[0] = reply;
            },
            options
        );

        assertNotNull(receivedReply[0]);
        assertEquals(sample, ((Reply.Success) receivedReply[0]).getSample());

        queryable.close();
        querier.close();
        session.close();
    }
}
