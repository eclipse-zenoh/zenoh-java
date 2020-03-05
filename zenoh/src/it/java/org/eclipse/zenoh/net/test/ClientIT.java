/*
 * Copyright (c) 2017, 2020 ADLINK Technology Inc.
 *
 * This program and the accompanying materials are made available under the
 * terms of the Eclipse Public License 2.0 which is available at
 * http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
 * which is available at https://www.apache.org/licenses/LICENSE-2.0.
 *
 * SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
 *
 * Contributors:
 *   ADLINK zenoh team, <zenoh@adlink-labs.tech>
 */

package org.eclipse.zenoh.net.test;

import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import org.junit.Assert;
import org.junit.Test;

import org.eclipse.zenoh.net.*;

public class ClientIT {

    public static class MVar<T> {
        T ref;
        T val;

        public synchronized T get() {
            try {
                while (val == null) {
                    wait();
                }
                T result = val;
                val = null;
                notify();
                return result;
            } catch (InterruptedException e) {
                return null;
            }
        }

        public synchronized void put(T putval) {
            try {
                while (val != null) {
                    wait();
                }
                val = putval;
                ref = putval;
                notify();
            } catch (InterruptedException e) {
            }
        }

        public T read() {
            return ref;
        }
    }

    public static boolean resourceEquals(Resource r1, Resource r2) {
        return r1.getRname().equals(r2.getRname()) && r1.getData().equals(r2.getData());
    }

    MVar<Resource> z1_sub1_last_res = new MVar<Resource>();
    MVar<Resource> z2_sub1_last_res = new MVar<Resource>();
    MVar<Resource> z3_sub1_last_res = new MVar<Resource>();
    MVar<Resource> z1_sto1_last_res = new MVar<Resource>();
    MVar<Resource> z2_sto1_last_res = new MVar<Resource>();
    MVar<List<List<Resource>>> replies = new MVar<List<List<Resource>>>();

    private class z1_sub1_listener implements DataHandler {
        public void handleData(String rname, ByteBuffer data, DataInfo info) {
            z1_sub1_last_res.put(new Resource(rname, data, info.getEncoding(), info.getKind()));
        }
    }

    private class z2_sub1_listener implements DataHandler {
        public void handleData(String rname, ByteBuffer data, DataInfo info) {
            z2_sub1_last_res.put(new Resource(rname, data, info.getEncoding(), info.getKind()));
        }
    }

    private class z3_sub1_listener implements DataHandler {
        public void handleData(String rname, ByteBuffer data, DataInfo info) {
            z3_sub1_last_res.put(new Resource(rname, data, info.getEncoding(), info.getKind()));
        }
    }

    private class z1_sto1_listener implements StorageHandler {
        public void handleData(String rname, ByteBuffer data, DataInfo info) {
            z1_sto1_last_res.put(new Resource(rname, data, info.getEncoding(), info.getKind()));
        }

        public void handleQuery(String rname, String predicate, RepliesSender repliesSender) {
            repliesSender.sendReplies(new Resource[] { z1_sto1_last_res.read() });
        }
    }

    private class z2_sto1_listener implements StorageHandler {
        public void handleData(String rname, ByteBuffer data, DataInfo info) {
            z2_sto1_last_res.put(new Resource(rname, data, info.getEncoding(), info.getKind()));
        }

        public void handleQuery(String rname, String predicate, RepliesSender repliesSender) {
            repliesSender.sendReplies(new Resource[] { z2_sto1_last_res.read() });
        }
    }

    private class z1_eval1_handler implements QueryHandler {
        public void handleQuery(String rname, String predicate, RepliesSender repliesSender) {
            try {
                ByteBuffer data = (ByteBuffer) ByteBuffer.allocateDirect(256).put("z1_eval1_data".getBytes("UTF-8"))
                        .flip();
                repliesSender.sendReplies(new Resource[] { new Resource("/test/java/client/z1_eval1", data, 0, 0) });
            } catch (Exception e) {
                repliesSender.sendReplies(new Resource[0]);
            }
        }
    }

    private class z2_eval1_handler implements QueryHandler {
        public void handleQuery(String rname, String predicate, RepliesSender repliesSender) {
            try {
                ByteBuffer data = (ByteBuffer) ByteBuffer.allocateDirect(256).put("z2_eval1_data".getBytes("UTF-8"))
                        .flip();
                repliesSender.sendReplies(new Resource[] { new Resource("/test/java/client/z2_eval1", data, 0, 0) });
            } catch (Exception e) {
                repliesSender.sendReplies(new Resource[0]);
            }
        }
    }

    private class reply_handler implements ReplyHandler {
        List<Resource> storage_replies = new ArrayList<Resource>();
        List<Resource> eval_replies = new ArrayList<Resource>();

        public void handleReply(ReplyValue reply) {
            switch (reply.getKind()) {
            case ZN_STORAGE_DATA:
                storage_replies.add(new Resource(reply.getRname(), reply.getData(), reply.getInfo().getKind(),
                        reply.getInfo().getEncoding()));
                break;
            case ZN_EVAL_DATA:
                eval_replies.add(new Resource(reply.getRname(), reply.getData(), reply.getInfo().getKind(),
                        reply.getInfo().getEncoding()));
                break;
            case ZN_REPLY_FINAL:
                replies.put(Arrays.asList(storage_replies, eval_replies));
                storage_replies = new ArrayList<Resource>();
                eval_replies = new ArrayList<Resource>();
                break;
            default:
                break;
            }
        }
    }

    @Test
    public final void test() throws Exception {

        System.out.println("============================");

        Session z1 = Session.open("tcp/127.0.0.1:7447");
        Subscriber z1_sub1 = z1.declareSubscriber("/test/java/client/**", SubMode.push(), new z1_sub1_listener());
        Storage z1_sto1 = z1.declareStorage("/test/java/client/**", new z1_sto1_listener());
        Eval z1_eval1 = z1.declareEval("/test/java/client/z1_eval1", new z1_eval1_handler());
        Publisher z1_pub1 = z1.declarePublisher("/test/java/client/z1_pub1");

        Session z2 = Session.open("tcp/127.0.0.1:7447");
        Subscriber z2_sub1 = z2.declareSubscriber("/test/java/client/**", SubMode.push(), new z2_sub1_listener());
        Storage z2_sto1 = z2.declareStorage("/test/java/client/**", new z2_sto1_listener());
        Eval z2_eval1 = z2.declareEval("/test/java/client/z2_eval1", new z2_eval1_handler());
        Publisher z2_pub1 = z2.declarePublisher("/test/java/client/z2_pub1");

        Session z3 = Session.open("tcp/127.0.0.1:7447");
        Subscriber z3_sub1 = z3.declareSubscriber("/test/java/client/**", SubMode.pull(), new z3_sub1_listener());

        Thread.sleep(1000);

        ByteBuffer sent_buf;
        Resource sent_res;
        Resource rcvd_res;
        List<List<Resource>> reps;

        sent_buf = (ByteBuffer) ByteBuffer.allocateDirect(256).put("z1_wr1_spl1".getBytes("UTF-8")).flip();
        sent_res = new Resource("/test/java/client/z1_wr1", sent_buf, 0, 0);
        z1.writeData(sent_res.getRname(), sent_res.getData().duplicate());
        rcvd_res = z1_sub1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        rcvd_res = z2_sub1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        rcvd_res = z1_sto1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        rcvd_res = z2_sto1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        z3_sub1.pull();
        rcvd_res = z3_sub1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));

        z1.query("/test/java/client/**", "", new reply_handler());
        reps = replies.get();
        Assert.assertEquals(2, reps.get(0).size());
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(0)));
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(1)));
        Assert.assertEquals(2, reps.get(1).size());

        z1.query("/test/java/client/**", "", new reply_handler(), QueryDest.bestMatch(), QueryDest.none());
        reps = replies.get();
        Assert.assertEquals(2, reps.get(0).size());
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(0)));
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(1)));
        // self.assertEqual(0, len(eval_replies))
        // This may not be true for now as :
        // - zenoh-c does not check received query properties
        // - zenohd does not filter out replies

        z1.query("/test/java/client/**", "", new reply_handler(), QueryDest.none(), QueryDest.bestMatch());
        reps = replies.get();
        // Assert.assertEquals(0, reps.get(0).size());
        // This may not be true for now as :
        // - zenoh-c does not check received query properties
        // - zenohd does not filter out replies
        Assert.assertEquals(2, reps.get(1).size());

        z2.query("/test/java/client/**", "", new reply_handler());
        reps = replies.get();
        Assert.assertEquals(2, reps.get(0).size());
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(0)));
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(1)));
        Assert.assertEquals(2, reps.get(1).size());

        z2.query("/test/java/client/**", "", new reply_handler(), QueryDest.bestMatch(), QueryDest.none());
        reps = replies.get();
        Assert.assertEquals(2, reps.get(0).size());
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(0)));
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(1)));
        // self.assertEqual(0, len(eval_replies))
        // This may not be true for now as :
        // - zenoh-c does not check received query properties
        // - zenohd does not filter out replies

        z2.query("/test/java/client/**", "", new reply_handler(), QueryDest.none(), QueryDest.bestMatch());
        reps = replies.get();
        // Assert.assertEquals(0, reps.get(0).size());
        // This may not be true for now as :
        // - zenoh-c does not check received query properties
        // - zenohd does not filter out replies
        Assert.assertEquals(2, reps.get(1).size());

        sent_buf = (ByteBuffer) ByteBuffer.allocateDirect(256).put("z2_wr1_spl1".getBytes("UTF-8")).flip();
        sent_res = new Resource("/test/java/client/**", sent_buf, 0, 0);
        z2.writeData(sent_res.getRname(), sent_res.getData().duplicate());
        rcvd_res = z1_sub1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        rcvd_res = z2_sub1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        rcvd_res = z1_sto1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        rcvd_res = z2_sto1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        z3_sub1.pull();
        rcvd_res = z3_sub1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));

        z1.query("/test/java/client/**", "", new reply_handler());
        reps = replies.get();
        Assert.assertEquals(2, reps.get(0).size());
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(0)));
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(1)));
        Assert.assertEquals(2, reps.get(1).size());

        z2.query("/test/java/client/**", "", new reply_handler());
        reps = replies.get();
        Assert.assertEquals(2, reps.get(0).size());
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(0)));
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(1)));
        Assert.assertEquals(2, reps.get(1).size());

        sent_buf = (ByteBuffer) ByteBuffer.allocateDirect(256).put("z1_pub1_spl1".getBytes("UTF-8")).flip();
        sent_res = new Resource("/test/java/client/z1_pub1", sent_buf, 0, 0);
        z1_pub1.streamData(sent_res.getData().duplicate());
        rcvd_res = z1_sub1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        rcvd_res = z2_sub1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        rcvd_res = z1_sto1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        rcvd_res = z2_sto1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        z3_sub1.pull();
        rcvd_res = z3_sub1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));

        z1.query("/test/java/client/**", "", new reply_handler());
        reps = replies.get();
        Assert.assertEquals(2, reps.get(0).size());
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(0)));
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(1)));
        Assert.assertEquals(2, reps.get(1).size());

        z2.query("/test/java/client/**", "", new reply_handler());
        reps = replies.get();
        Assert.assertEquals(2, reps.get(0).size());
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(0)));
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(1)));
        Assert.assertEquals(2, reps.get(1).size());

        sent_buf = (ByteBuffer) ByteBuffer.allocateDirect(256).put("z2_pub1_spl1".getBytes("UTF-8")).flip();
        sent_res = new Resource("/test/java/client/z2_pub1", sent_buf, 0, 0);
        z2_pub1.streamData(sent_res.getData().duplicate());
        rcvd_res = z1_sub1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        rcvd_res = z2_sub1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        rcvd_res = z1_sto1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        rcvd_res = z2_sto1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));
        z3_sub1.pull();
        rcvd_res = z3_sub1_last_res.get();
        Assert.assertTrue(resourceEquals(sent_res, rcvd_res));

        z1.query("/test/java/client/**", "", new reply_handler());
        reps = replies.get();
        Assert.assertEquals(2, reps.get(0).size());
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(0)));
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(1)));
        Assert.assertEquals(2, reps.get(1).size());

        z2.query("/test/java/client/**", "", new reply_handler());
        reps = replies.get();
        Assert.assertEquals(2, reps.get(0).size());
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(0)));
        Assert.assertTrue(resourceEquals(sent_res, reps.get(0).get(1)));
        Assert.assertEquals(2, reps.get(1).size());

        z1_sub1.undeclare();
        z2_sub1.undeclare();
        z3_sub1.undeclare();
        z1_sto1.undeclare();
        z2_sto1.undeclare();
        z1_eval1.undeclare();
        z2_eval1.undeclare();
        z1_pub1.undeclare();
        z2_pub1.undeclare();

        z1.close();
        z2.close();
        z3.close();
    }
}
