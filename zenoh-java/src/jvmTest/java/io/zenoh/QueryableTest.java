package io.zenoh;

import io.zenoh.bytes.Encoding;
import io.zenoh.bytes.ZBytes;
import io.zenoh.exceptions.ZError;
import io.zenoh.handlers.Handler;
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.query.*;
import io.zenoh.qos.CongestionControl;
import io.zenoh.qos.Priority;
import io.zenoh.qos.QoS;
import io.zenoh.sample.Sample;
import io.zenoh.sample.SampleKind;
import org.apache.commons.net.ntp.TimeStamp;
import org.junit.After;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Date;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.Assert.*;

@RunWith(JUnit4.class)
public class QueryableTest {

    private static final ZBytes testPayload = ZBytes.from("Hello queryable");
    private Session session;
    private KeyExpr testKeyExpr;

    @Before
    public void setUp() throws ZError {
        session = Zenoh.open(Config.loadDefault());
        testKeyExpr = KeyExpr.tryFrom("example/testing/keyexpr");
    }

    @After
    public void tearDown() throws ZError {
        session.close();
        testKeyExpr.close();
    }

    @Test
    public void queryableRunsWithCallback() throws ZError {
        var timestamp = new TimeStamp(Date.from(Instant.now()));

        var sample = new Sample(
                testKeyExpr,
                testPayload,
                Encoding.defaultEncoding(),
                SampleKind.PUT,
                timestamp,
                QoS.defaultQoS(),
                null
        );

        var queryable = session.declareQueryable(testKeyExpr, new QueryableCallbackConfig(query ->
        {
            try {
                query.reply(testKeyExpr, testPayload, new ReplyConfig()
                        .timestamp(timestamp)
                        .congestionControl(QoS.defaultQoS().getCongestionControl())
                        .priority(QoS.defaultQoS().getPriority())
                        .express(QoS.defaultQoS().getExpress()));
            } catch (ZError e) {
                throw new RuntimeException(e);
            }
        }
        ));

        Reply[] reply = new Reply[1];
        session.get(testKeyExpr.into()).callback(reply1 -> reply[0] = reply1).timeout(Duration.ofMillis(1000)).res();

        assertNotNull(reply[0]);
        Sample receivedSample = ((Reply.Success) reply[0]).getSample();
        assertEquals(sample, receivedSample);
        queryable.close();
    }

    @Test
    public void queryableRunsWithHandler() throws ZError, InterruptedException {
        var config = new QueryableHandlerConfig<>(new QueryHandler());
        var queryable = session.declareQueryable(testKeyExpr, config);

        Thread.sleep(500);

        Reply[] reply = new Reply[1];
        session.get(testKeyExpr.into()).callback(reply1 -> reply[0] = reply1).res();

        Thread.sleep(500);

        queryable.close();
        assertTrue(reply[0] instanceof Reply.Success);
    }

    @Test
    public void queryTest() throws ZError, InterruptedException {
        Query[] receivedQuery = new Query[1];
        var config = new QueryableCallbackConfig(query -> receivedQuery[0] = query);
        var queryable = session.declareQueryable(testKeyExpr, config);

        session.get(testKeyExpr).res();

        Thread.sleep(100);

        Query query = receivedQuery[0];
        assertNotNull(query);
        assertNull(query.getPayload());
        assertNull(query.getEncoding());
        assertNull(query.getAttachment());

        receivedQuery[0] = null;
        var payload = ZBytes.from("Test value");
        var attachment = ZBytes.from("Attachment");
        session.get(testKeyExpr).callback(reply -> {
        }).payload(payload).encoding(Encoding.ZENOH_STRING).attachment(attachment).res();

        Thread.sleep(100);

        query = receivedQuery[0];
        assertNotNull(query);
        assertEquals(payload, query.getPayload());
        assertEquals(Encoding.ZENOH_STRING, query.getEncoding());
        assertEquals(attachment, query.getAttachment());

        queryable.close();
    }

    @Test
    public void queryReplySuccessTest() throws ZError {
        var message = ZBytes.from("Test message");
        var timestamp = TimeStamp.getCurrentTime();
        QueryableCallbackConfig config = new QueryableCallbackConfig(query -> {
            try {
                query.reply(testKeyExpr, message, new ReplyConfig()
                        .timestamp(timestamp)
                        .priority(Priority.DATA_HIGH)
                        .express(true)
                        .congestionControl(CongestionControl.DROP));
            } catch (ZError e) {
                throw new RuntimeException(e);
            }
        });

        Queryable<Void> queryable = session.declareQueryable(testKeyExpr, config);

        Reply[] receivedReply = new Reply[1];
        session.get(testKeyExpr).callback(reply -> receivedReply[0] = reply).timeout(Duration.ofMillis(10)).res();

        queryable.close();

        assertNotNull(receivedReply[0]);
        assertTrue(receivedReply[0] instanceof Reply.Success);

        var sample = ((Reply.Success) receivedReply[0]).getSample();
        assertEquals(message, sample.getPayload());
        assertEquals(timestamp, sample.getTimestamp());
        assertEquals(Priority.DATA_HIGH, sample.getQos().getPriority());
        assertTrue(sample.getQos().getExpress());
        assertEquals(CongestionControl.DROP, sample.getQos().getCongestionControl());
    }

    @Test
    public void queryReplyErrorTest() throws ZError, InterruptedException {
        var errorMessage = ZBytes.from("Error message");

        var queryable = session.declareQueryable(testKeyExpr, new QueryableCallbackConfig(query ->
        {
            try {
                query.replyErr(errorMessage);
            } catch (ZError e) {
                throw new RuntimeException(e);
            }
        }
        ));

        Reply[] receivedReply = new Reply[1];
        session.get(testKeyExpr).callback(reply -> receivedReply[0] = reply).timeout(Duration.ofMillis(10)).res();

        Thread.sleep(1000);
        queryable.close();

        assertNotNull(receivedReply[0]);
        assertTrue(receivedReply[0] instanceof Reply.Error);

        var errorReply = (Reply.Error) receivedReply[0];
        assertEquals(errorMessage, errorReply.getError());
    }

    @Test
    public void queryReplyDeleteTest() throws ZError, InterruptedException {
        var timestamp = TimeStamp.getCurrentTime();

        var queryable = session.declareQueryable(testKeyExpr, new QueryableCallbackConfig(query -> {
            try {
                var config = new ReplyDelConfig();
                config.setTimeStamp(timestamp);
                query.replyDel(testKeyExpr, config);
            } catch (ZError e) {
                throw new RuntimeException(e);
            }
        }));

        Reply[] receivedReply = new Reply[1];
        session.get(testKeyExpr).callback(reply -> receivedReply[0] = reply).timeout(Duration.ofMillis(10)).res();

        Thread.sleep(1000);
        queryable.close();

        assertNotNull(receivedReply[0]);
        assertTrue(receivedReply[0] instanceof Reply.Success);

        var sample = ((Reply.Success) receivedReply[0]).getSample();
        assertEquals(SampleKind.DELETE, sample.getKind());
        assertEquals(timestamp, sample.getTimestamp());
    }

    @Test
    public void onCloseTest() throws InterruptedException, ZError {
        AtomicReference<Boolean> onCloseWasCalled = new AtomicReference<>(false);
        var queryable = session.declareQueryable(testKeyExpr, new QueryableCallbackConfig(query -> {
        }).onClose(() -> onCloseWasCalled.set(true)));
        queryable.undeclare();

        Thread.sleep(1000);
        assertTrue(onCloseWasCalled.get());
    }
}

class QueryHandler implements Handler<Query, QueryHandler> {

    private int counter = 0;
    private final ArrayList<Sample> performedReplies = new ArrayList<>();

    public ArrayList<Sample> getPerformedReplies() {
        return performedReplies;
    }

    @Override
    public void handle(Query query) {
        try {
            reply(query);
        } catch (ZError e) {
            throw new RuntimeException(e);
        }
    }

    @Override
    public QueryHandler receiver() {
        return this;
    }

    @Override
    public void onClose() {
        // No action needed on close
    }

    public void reply(Query query) throws ZError {
        ZBytes payload = ZBytes.from("Hello queryable " + counter + "!");
        counter++;
        Sample sample = new Sample(
                query.getKeyExpr(),
                payload,
                Encoding.defaultEncoding(),
                SampleKind.PUT,
                new TimeStamp(Date.from(Instant.now())),
                new QoS(),
                null
        );
        performedReplies.add(sample);
        var config = new ReplyConfig();
        config.setTimeStamp(sample.getTimestamp());
        query.reply(query.getKeyExpr(), payload, config);
    }
}
