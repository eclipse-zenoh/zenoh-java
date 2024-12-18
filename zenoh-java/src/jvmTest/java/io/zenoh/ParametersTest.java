package io.zenoh;

import io.zenoh.query.Parameters;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import java.util.List;
import java.util.Map;

import static org.junit.Assert.*;

@RunWith(JUnit4.class)
public class ParametersTest {

    @Test
    public void shouldCreateEmptyParametersFromEmptyString() {
        var result = Parameters.from("");

        assertTrue(result.isEmpty());
    }

    @Test
    public void shouldParseParametersFromFormattedString() {
        var parameters = Parameters.from("a=1;b=2;c=3|4|5;d=6");

        assertEquals("1", parameters.get("a"));
        assertEquals("2", parameters.get("b"));
        assertEquals("3|4|5", parameters.get("c"));
        assertEquals("6", parameters.get("d"));
    }

    @Test
    public void shouldReturnListOfValuesSplitBySeparator() {
        var parameters = Parameters.from("a=1;b=2;c=3|4|5;d=6");

        assertEquals(List.of("3", "4", "5"), parameters.values("c"));
    }

    @Test
    public void containsKeyTest() {
        var parameters = Parameters.from("a=1;b=2;c=3|4|5;d=6");

        assertTrue(parameters.containsKey("a"));
        assertFalse(parameters.containsKey("e"));
    }

    @Test
    public void getTest() {
        var parameters = Parameters.from("a=1;b=2;c=3|4|5;d=6");

        assertEquals("1", parameters.get("a"));
        assertEquals("2", parameters.get("b"));
        assertEquals("3|4|5", parameters.get("c"));
        assertEquals("6", parameters.get("d"));
    }

    @Test
    public void getOrDefaultTest() {
        var parameters = Parameters.from("a=1;b=2;c=3|4|5;d=6");

        assertEquals("1", parameters.get("a"));
        assertEquals("None", parameters.getOrDefault("e", "None"));
    }

    @Test
    public void toMapTest() {
        var parameters = Parameters.from("a=1;b=2;c=3|4|5;d=6");

        assertEquals(Map.of("a", "1", "b", "2", "c", "3|4|5", "d", "6"), parameters.toMap());
    }

    @Test
    public void insertShouldReturnPreviouslyContainedValue() {
        var parameters = Parameters.from("a=1");
        var oldValue = parameters.insert("a", "3");
        assertEquals("1", oldValue);
    }

    @Test
    public void insertShouldReturnNullIfNotAlreadyPresent() {
        var parameters = Parameters.empty();
        var oldValue = parameters.insert("a", "1");
        assertNull(oldValue);
    }

    @Test
    public void removeShouldReturnOldValueIfPresent() {
        var parameters = Parameters.from("a=1");
        var oldValue = parameters.remove("a");
        assertEquals("1", oldValue);
    }

    @Test
    public void removeShouldReturnNullIfNotAlreadyPresent() {
        var parameters = Parameters.empty();
        var oldValue = parameters.remove("a");
        assertNull(oldValue);
    }

    @Test
    public void extendTest() {
        var parameters = Parameters.from("a=1;b=2");
        parameters.extend(Parameters.from("c=3;d=4"));

        assertEquals(Parameters.from("a=1;b=2;c=3;d=4"), parameters);

        parameters.extend(Map.of("e", "5"));
        assertEquals(Parameters.from("a=1;b=2;c=3;d=4;e=5"), parameters);
    }

    @Test
    public void extendOverwritesConflictingKeysTest() {
        var parameters = Parameters.from("a=1;b=2");
        parameters.extend(Parameters.from("b=3;d=4"));

        assertEquals(Parameters.from("a=1;b=3;d=4"), parameters);
    }

    @Test
    public void emptyParametersToStringTest() {
        var parameters = Parameters.empty();
        assertEquals("", parameters.toString());
    }
}
