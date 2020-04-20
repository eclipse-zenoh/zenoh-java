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

import org.eclipse.zenoh.Path;
import org.eclipse.zenoh.RawValue;
import org.eclipse.zenoh.Value;
import org.eclipse.zenoh.Workspace;
import org.eclipse.zenoh.Zenoh;

import picocli.CommandLine;
import picocli.CommandLine.Option;

class ZPutThr implements Runnable {

    @Option(names = {"-h", "--help"}, usageHelp = true, description = "display this help message")
    private boolean helpRequested = false;

    @Option(names = {"-s", "--size"},
        description = "the size in bytes of the payload used for the throughput test. [default: ${DEFAULT-VALUE}]")
    private int size = 256;

    @Option(names = {"-l", "--locator"},
        description = "The locator to be used to boostrap the zenoh session. By default dynamic discovery is used")
    private String locator = null;

    @Option(names = {"-p", "--path"},
        description = "the resource used to write throughput data.\n  [default: ${DEFAULT-VALUE}]")
    private String path = "/zenoh/examples/throughput/data";

    @Option(names = {"-b", "--buffer"},
    description = "The type of ByteBuffer to use. Valid values: ${COMPLETION-CANDIDATES}. [default: ${DEFAULT-VALUE}]")
    private BufferKind bufferKind = BufferKind.DIRECT;

    private enum BufferKind {
        DIRECT,
        NON_DIRECT,
        WRAPPED,
    }

    @Override
    public void run() {
        java.nio.ByteBuffer data = null;
        switch (bufferKind) {
            case DIRECT:
                data = java.nio.ByteBuffer.allocateDirect(size);
                System.out.println("Running throughput test for payload of " + size + " bytes from a direct ByteBuffer");
                break;
            case NON_DIRECT:
                data = java.nio.ByteBuffer.allocate(size);
                System.out.println("Running throughput test for payload of " + size + " bytes from a non-direct ByteBuffer");
                break;
            case WRAPPED: 
                // allocate more than len, to wrap with an offset and test the impact
                byte[] array = new byte[size + 1024];
                data = java.nio.ByteBuffer.wrap(array, 100, size);
                System.out.println("Running throughput test for payload of " + size + " bytes from a wrapped ByteBuffer");
                break;
        }

        int posInit = data.position();
        for (int i = 0; i < size; ++i) {
            data.put((byte) (i % 10));
        }
        data.flip();
        data.position(posInit);

        try {
            Path p = new Path(path);

            Value v = new RawValue(data);

            System.out.println("Login to Zenoh (locator=" + locator + ")...");
            Zenoh z = Zenoh.login(locator);

            Workspace w = z.workspace();

            System.out.println("Put on " + p + " : " + data.remaining() + "b");

            while (true) {
                w.put(p, v);
            }

        } catch (Throwable e) {
            e.printStackTrace();
        }
    }

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZPutThr()).execute(args);
        System.exit(exitCode);
    }
}
