package io.zenoh;

import io.zenoh.bytes.ZBytes;
import io.zenoh.exceptions.ZError;
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.bytes.Encoding;
import io.zenoh.pubsub.PublisherOptions;
import io.zenoh.pubsub.PutOptions;
import io.zenoh.qos.QoS;
import io.zenoh.qos.Reliability;
import io.zenoh.sample.SampleKind;
import io.zenoh.pubsub.Publisher;
import io.zenoh.sample.Sample;
import io.zenoh.pubsub.Subscriber;
import kotlin.Pair;
import org.junit.After;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import java.util.ArrayList;
import java.util.List;

import static org.junit.Assert.assertEquals;

@RunWith(JUnit4.class)
public class PublisherTest {

    private Session session;
    private ArrayList<Sample> receivedSamples;
    private Publisher publisher;
    private Subscriber subscriber;
    private KeyExpr keyExpr;

    @Before
    public void setUp() throws ZError {
        session = Zenoh.open(Config.loadDefault());
        keyExpr = KeyExpr.tryFrom("example/testing/keyexpr");

        var config = new PublisherOptions();
        config.setReliability(Reliability.RELIABLE);
        config.setEncoding(Encoding.ZENOH_STRING);
        publisher = session.declarePublisher(keyExpr, config);

        receivedSamples = new ArrayList<>();

        subscriber = session.declareSubscriber(keyExpr, receivedSamples::add);
    }

    @After
    public void tearDown() throws ZError {
        publisher.close();
        subscriber.close();
        session.close();
        keyExpr.close();
    }

    @Test
    public void putTest() {

        List<Pair<ZBytes, Encoding>> testPayloads = List.of(
                new Pair<>(ZBytes.from("Test 1"), Encoding.TEXT_PLAIN),
                new Pair<>(ZBytes.from("Test 2"), Encoding.TEXT_JSON),
                new Pair<>(ZBytes.from("Test 3"), Encoding.TEXT_CSV)
        );

        testPayloads.forEach(value -> {
            try {
                var putOptions = new PutOptions();
                putOptions.setEncoding(value.getSecond());
                publisher.put(value.getFirst(), putOptions);
            } catch (ZError e) {
                throw new RuntimeException(e);
            }
        });

        assertEquals(testPayloads.size(), receivedSamples.size());
        for (int index = 0; index < receivedSamples.size(); index++) {
            var sample = receivedSamples.get(index);
            assertEquals(testPayloads.get(index).getFirst(), sample.getPayload());
            assertEquals(testPayloads.get(index).getSecond(), sample.getEncoding());
        }
    }

    @Test
    public void deleteTest() throws ZError {
        publisher.delete();
        assertEquals(1, receivedSamples.size());
        assertEquals(SampleKind.DELETE, receivedSamples.get(0).getKind());
    }

    @Test
    public void shouldFallbackToPublisherEncodingWhenEncodingNotProvided() throws ZError {
        publisher.put(ZBytes.from("Test"));
        assertEquals(1, receivedSamples.size());
        assertEquals(Encoding.ZENOH_STRING, receivedSamples.get(0).getEncoding());
    }
}
