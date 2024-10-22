package io.zenoh

import io.zenoh.exceptions.ZError
import io.zenoh.query.Selector
import io.zenoh.query.intoSelector
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class SelectorTest {

    init {
        Zenoh.load()
    }

    @Test
    fun selectorFromStringTest() {
        "a/b/c?arg1=val1".intoSelector().use { selector: Selector ->
            assertEquals("a/b/c", selector.keyExpr.toString())
            assertEquals("arg1=val1", selector.parameters)
        }

        "a/b/c".intoSelector().use { selector: Selector ->
            assertEquals("a/b/c", selector.keyExpr.toString())
            assertEquals("", selector.parameters)
        }

        assertFailsWith<ZError> { "".intoSelector() }
    }
}
