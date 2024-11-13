package io.zenoh;

import io.zenoh.bytes.ZBytes;
import io.zenoh.exceptions.ZError;
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.bytes.Encoding;
import io.zenoh.sample.SampleKind;
import io.zenoh.pubsub.Publisher;
import io.zenoh.sample.Sample;
import io.zenoh.pubsub.Subscriber;
import io.zenoh.Config;
import io.zenoh.Session;
import kotlin.Pair;
import kotlin.Unit;
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
    private Subscriber<Unit> subscriber;
    private KeyExpr keyExpr;

    @Before
    public void setUp() throws ZError {
        session = Zenoh.open(Config.loadDefault());
        keyExpr = KeyExpr.tryFrom("example/testing/keyexpr");
        publisher = session.declarePublisher(keyExpr).encoding(Encoding.ZENOH_STRING).res();
        receivedSamples = new ArrayList<>();
        subscriber = session.declareSubscriber(keyExpr).callback( sample -> receivedSamples.add(sample)).res();
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
                publisher.put(value.getFirst()).encoding(value.getSecond()).res();
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
        publisher.delete().res();
        assertEquals(1, receivedSamples.size());
        assertEquals(SampleKind.DELETE, receivedSamples.get(0).getKind());
    }

    @Test
    public void shouldFallbackToPublisherEncodingWhenEncodingNotProvided() throws ZError {
        publisher.put(ZBytes.from("Test")).res();
        assertEquals(1, receivedSamples.size());
        assertEquals(Encoding.ZENOH_STRING, receivedSamples.get(0).getEncoding());
    }
}
