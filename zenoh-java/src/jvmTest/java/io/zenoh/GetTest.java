package io.zenoh;

import io.zenoh.bytes.ZBytes;
import io.zenoh.exceptions.ZError;
import io.zenoh.handlers.Handler;
import io.zenoh.query.*;
import io.zenoh.sample.Sample;
import io.zenoh.sample.SampleKind;
import org.apache.commons.net.ntp.TimeStamp;
import org.junit.After;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import java.time.Duration;
import java.util.ArrayList;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;

@RunWith(JUnit4.class)
public class GetTest {

    static final ZBytes payload = ZBytes.from("test");
    static final TimeStamp timestamp = TimeStamp.getCurrentTime();
    static final SampleKind kind = SampleKind.PUT;

    private Session session;
    private Selector selector;
    private Queryable<Void> queryable;

    @Before
    public void setUp() throws ZError {
        session = Zenoh.open(Config.loadDefault());
        selector = Selector.tryFrom("example/testing/keyexpr");
        queryable = session.declareQueryable(selector.getKeyExpr(), query ->
                {
                    try {
                        var options = new ReplyOptions();
                        options.setTimeStamp(timestamp);
                        query.reply(query.getKeyExpr(), payload, options);
                    } catch (ZError e) {
                        throw new RuntimeException(e);
                    }
                }
        );
    }

    @After
    public void tearDown() throws ZError {
        session.close();
        selector.close();
        queryable.close();
    }

    @Test
    public void get_runsWithCallbackTest() throws ZError {
        Reply[] reply = new Reply[1];

        var getOptions = new GetOptions();
        getOptions.setTimeout(Duration.ofMillis(1000));
        session.get(selector, reply1 -> reply[0] = reply1, getOptions);

        assertNotNull(reply[0]);
        Sample sample = ((Reply.Success) reply[0]).getSample();
        assertEquals(payload, sample.getPayload());
        assertEquals(kind, sample.getKind());
        assertEquals(selector.getKeyExpr(), sample.getKeyExpr());
        assertEquals(timestamp, sample.getTimestamp());
    }

    @Test
    public void get_runsWithHandlerTest() throws ZError {
        var getOptions = new GetOptions();
        getOptions.setTimeout(Duration.ofMillis(1000));
        ArrayList<Reply> receiver = session.get(selector, new TestHandler(), getOptions);
        for (Reply reply : receiver) {
            Sample sample = ((Reply.Success) reply).getSample();
            assertEquals(payload, sample.getPayload());
            assertEquals(SampleKind.PUT, sample.getKind());
        }
    }

    @Test
    public void getWithSelectorParamsTest() throws ZError {
        Parameters[] receivedParams = new Parameters[1];

        Queryable<Void> queryable = session.declareQueryable(selector.getKeyExpr(), query ->
                receivedParams[0] = query.getParameters()
        );

        Parameters params = Parameters.from("arg1=val1&arg2=val2&arg3");
        Selector selectorWithParams = new Selector(selector.getKeyExpr(), params);
        var getOptions = new GetOptions();
        getOptions.setTimeout(Duration.ofMillis(1000));
        session.get(selectorWithParams, getOptions);

        queryable.close();

        assertEquals(params, receivedParams[0]);
    }
}

/**
 * A dummy handler for get operations.
 */
class TestHandler implements Handler<Reply, ArrayList<Reply>> {

    static final ArrayList<Reply> performedReplies = new ArrayList<>();

    @Override
    public void handle(Reply t) {
        performedReplies.add(t);
    }

    @Override
    public ArrayList<Reply> receiver() {
        return performedReplies;
    }

    @Override
    public void onClose() {
    }
}
