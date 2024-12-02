package io.zenoh;

import io.zenoh.handlers.Handler;

import java.util.ArrayDeque;

/**
 * Sample handler for the sake of the examples.
 *
 * A custom handler can be implemented to handle incoming samples, queries or replies for
 * subscribers, get operations, query operations or queryables.
 *
 * The example below shows a queue handler, in which an ArrayDeque is specified as the receiver type.
 * The function handle will be called everytime an element of type T is received and in our example
 * implementation, elements are simply enqueued into the queue, which can later be retrieved.
 */
class QueueHandler<T extends ZenohType> implements Handler<T, ArrayDeque<T>> {

    final ArrayDeque<T> queue = new ArrayDeque<>();

    @Override
    public void handle(T t) {
        queue.add(t);
    }

    @Override
    public ArrayDeque<T> receiver() {
        return queue;
    }

    @Override
    public void onClose() {}
}
