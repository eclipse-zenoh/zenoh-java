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

import io.zenoh.bytes.ZBytes;
import io.zenoh.exceptions.ZError;
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.pubsub.Subscriber;
import io.zenoh.sample.Sample;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;

import static org.junit.Assert.*;

@RunWith(JUnit4.class)
public class ConfigTest {

    private static final String json5ClientConfigString =
            "{\n" +
                    "    mode: \"peer\",\n" +
                    "    connect: {\n" +
                    "        endpoints: [\"tcp/localhost:7450\"]\n" +
                    "    },\n" +
                    "    scouting: {\n" +
                    "        multicast: {\n" +
                    "            enabled: false\n" +
                    "        }\n" +
                    "    }\n" +
                    "}";

    private static final String json5ServerConfigString =
            "{\n" +
                    "    mode: \"peer\",\n" +
                    "    listen: {\n" +
                    "        endpoints: [\"tcp/localhost:7450\"],\n" +
                    "    },\n" +
                    "    scouting: {\n" +
                    "        multicast: {\n" +
                    "            enabled: false,\n" +
                    "        }\n" +
                    "    }\n" +
                    "}";

    private static final String jsonClientConfigString =
            "{\n" +
                    "    \"mode\": \"peer\",\n" +
                    "    \"connect\": {\n" +
                    "        \"endpoints\": [\"tcp/localhost:7450\"]\n" +
                    "    },\n" +
                    "    \"scouting\": {\n" +
                    "        \"multicast\": {\n" +
                    "            \"enabled\": false\n" +
                    "        }\n" +
                    "    }\n" +
                    "}";

    private static final String jsonServerConfigString =
            "{\n" +
                    "    \"mode\": \"peer\",\n" +
                    "    \"listen\": {\n" +
                    "        \"endpoints\": [\"tcp/localhost:7450\"]\n" +
                    "    },\n" +
                    "    \"scouting\": {\n" +
                    "        \"multicast\": {\n" +
                    "            \"enabled\": false\n" +
                    "        }\n" +
                    "    }\n" +
                    "}";

    private static final String yamlClientConfigString =
            "mode: peer\n" +
                    "connect:\n" +
                    "  endpoints:\n" +
                    "    - tcp/localhost:7450\n" +
                    "scouting:\n" +
                    "  multicast:\n" +
                    "    enabled: false\n";

    private static final String yamlServerConfigString =
            "mode: peer\n" +
                    "listen:\n" +
                    "  endpoints:\n" +
                    "    - tcp/localhost:7450\n" +
                    "scouting:\n" +
                    "  multicast:\n" +
                    "    enabled: false\n";

    private static final KeyExpr TEST_KEY_EXP;
    private static final Config json5ClientConfig;
    private static final Config json5ServerConfig;
    private static final Config jsonClientConfig;
    private static final Config jsonServerConfig;
    private static final Config yamlClientConfig;
    private static final Config yamlServerConfig;

    static {
        try {
            TEST_KEY_EXP = KeyExpr.tryFrom("example/testing/keyexpr");
            json5ClientConfig = Config.fromJson5(json5ClientConfigString);
            json5ServerConfig = Config.fromJson5(json5ServerConfigString);

            jsonClientConfig = Config.fromJson(jsonClientConfigString);
            jsonServerConfig = Config.fromJson(jsonServerConfigString);

            yamlClientConfig = Config.fromYaml(yamlClientConfigString);
            yamlServerConfig = Config.fromYaml(yamlServerConfigString);
        } catch (ZError e) {
            throw new RuntimeException(e);
        }
    }

    private void runSessionTest(Config clientConfig, Config serverConfig) throws ZError, InterruptedException {
        Session sessionClient = Zenoh.open(clientConfig);
        Session sessionServer = Zenoh.open(serverConfig);

        final Sample[] receivedSample = new Sample[1];

        Subscriber<Void> subscriber =
                sessionClient.declareSubscriber(TEST_KEY_EXP, sample -> receivedSample[0] = sample);
        ZBytes payload = ZBytes.from("example message");
        sessionClient.put(TEST_KEY_EXP, payload);

        Thread.sleep(1000);

        subscriber.close();
        sessionClient.close();
        sessionServer.close();

        assertNotNull(receivedSample[0]);
        assertEquals(receivedSample[0].getPayload(), payload);
    }

    @Test
    public void testConfigWithJSON5() throws ZError, InterruptedException {
        runSessionTest(json5ClientConfig, json5ServerConfig);
    }

    @Test
    public void testConfigLoadsFromJSONString() throws ZError, InterruptedException {
        runSessionTest(jsonClientConfig, jsonServerConfig);
    }


    @Test
    public void testConfigLoadsFromYAMLString() throws ZError, InterruptedException {
        runSessionTest(yamlClientConfig, yamlServerConfig);
    }

    @Test
    public void testDefaultConfig() throws ZError {
        Config config = Config.loadDefault();
        Session session = Zenoh.open(config);
        session.close();
    }

    @Test
    public void configFailsWithIllFormatedJsonTest() throws ZError {
        String illFormattedConfig =
                "{\n" +
                        "    mode: \"peer\",\n" +
                        "    connect: {\n" +
                        "        endpoints: [\"tcp/localhost:7450\"],\n" +
                        // missing '}' character here
                        "}";

        assertThrows(ZError.class, () -> Config.fromJson(illFormattedConfig));
    }

    @Test
    public void configFailsWithIllFormatedYAMLTest() {
        String illFormattedConfig =
                "mode: peer\n" +
                        "connect:\n" +
                        "  endpoints:\n" +
                        "    - tcp/localhost:7450\n" +
                        "scouting\n";

        assertThrows(ZError.class, () -> Config.fromJson(illFormattedConfig));
    }

    @Test
    public void configLoadsFromJSONFileTest() throws IOException, ZError, InterruptedException {
        File clientConfigFile = File.createTempFile("clientConfig", ".json");
        File serverConfigFile = File.createTempFile("serverConfig", ".json");

        try {
            // Writing text to the files
            Files.write(clientConfigFile.toPath(), jsonClientConfigString.getBytes());
            Files.write(serverConfigFile.toPath(), jsonServerConfigString.getBytes());

            Config loadedClientConfig = Config.fromFile(clientConfigFile);
            Config loadedServerConfig = Config.fromFile(serverConfigFile);

            runSessionTest(loadedClientConfig, loadedServerConfig);
        } finally {
            clientConfigFile.delete();
            serverConfigFile.delete();
        }
    }

    @Test
    public void configLoadsFromYAMLFileTest() throws IOException, ZError, InterruptedException {
        File clientConfigFile = File.createTempFile("clientConfig", ".yaml");
        File serverConfigFile = File.createTempFile("serverConfig", ".yaml");

        try {
            // Writing text to the files
            Files.write(clientConfigFile.toPath(), yamlClientConfigString.getBytes());
            Files.write(serverConfigFile.toPath(), yamlServerConfigString.getBytes());

            Config loadedClientConfig = Config.fromFile(clientConfigFile);
            Config loadedServerConfig = Config.fromFile(serverConfigFile);

            runSessionTest(loadedClientConfig, loadedServerConfig);
        } finally {
            clientConfigFile.delete();
            serverConfigFile.delete();
        }
    }

    @Test
    public void configLoadsFromJSON5FileTest() throws IOException, ZError, InterruptedException {
        File clientConfigFile = File.createTempFile("clientConfig", ".json5");
        File serverConfigFile = File.createTempFile("serverConfig", ".json5");

        try {
            // Writing text to the files
            Files.write(clientConfigFile.toPath(), json5ClientConfigString.getBytes());
            Files.write(serverConfigFile.toPath(), json5ServerConfigString.getBytes());

            Config loadedClientConfig = Config.fromFile(clientConfigFile);
            Config loadedServerConfig = Config.fromFile(serverConfigFile);

            runSessionTest(loadedClientConfig, loadedServerConfig);
        } finally {
            clientConfigFile.delete();
            serverConfigFile.delete();
        }
    }

    @Test
    public void configLoadsFromJSON5FileProvidingPathTest() throws IOException, ZError, InterruptedException {
        File clientConfigFile = File.createTempFile("clientConfig", ".json5");
        File serverConfigFile = File.createTempFile("serverConfig", ".json5");

        try {
            // Writing text to the files
            Files.write(clientConfigFile.toPath(), json5ClientConfigString.getBytes());
            Files.write(serverConfigFile.toPath(), json5ServerConfigString.getBytes());

            Config loadedClientConfig = Config.fromFile(clientConfigFile.toPath());
            Config loadedServerConfig = Config.fromFile(serverConfigFile.toPath());

            runSessionTest(loadedClientConfig, loadedServerConfig);
        } finally {
            clientConfigFile.delete();
            serverConfigFile.delete();
        }
    }

    @Test
    public void getJsonFunctionTest() throws ZError {
        String jsonConfig =
                "{\n" +
                        "    mode: \"peer\",\n" +
                        "    connect: {\n" +
                        "        endpoints: [\"tcp/localhost:7450\"],\n" +
                        "    },\n" +
                        "    scouting: {\n" +
                        "        multicast: {\n" +
                        "            enabled: false,\n" +
                        "        }\n" +
                        "    }\n" +
                        "}";

        Config config = Config.fromJson(jsonConfig);

        String value = config.getJson("connect");
        assertTrue(value.contains("\"endpoints\":[\"tcp/localhost:7450\"]"));

        String value2 = config.getJson("mode");
        assertEquals("\"peer\"", value2);
    }

    @Test
    public void configShouldRemainValidDespiteFailingToGetJsonValue() throws ZError {
        String jsonConfig =
                "{\n" +
                        "    mode: \"peer\",\n" +
                        "    connect: {\n" +
                        "        endpoints: [\"tcp/localhost:7450\"],\n" +
                        "    },\n" +
                        "    scouting: {\n" +
                        "        multicast: {\n" +
                        "            enabled: false,\n" +
                        "        }\n" +
                        "    }\n" +
                        "}";

        Config config = Config.fromJson(jsonConfig);

        assertThrows(ZError.class, () -> {
            config.getJson("non_existent_key");
        });

        String mode = config.getJson("mode");
        assertEquals("\"peer\"", mode);
    }

    @Test
    public void insertJson5FunctionTest() throws ZError {
        Config config = Config.loadDefault();
        String endpoints = "[\"tcp/8.8.8.8:8\", \"tcp/8.8.8.8:9\"]";

        config.insertJson5("listen/endpoints", endpoints);

        String jsonValue = config.getJson("listen/endpoints");
        assertTrue(jsonValue.contains("8.8.8.8"));
    }

    @Test
    public void insertIllFormattedJson5ShouldFailTest() throws ZError {
        Config config = Config.loadDefault();

        String illFormattedEndpoints = "[\"tcp/8.8.8.8:8\"";
        assertThrows(ZError.class, () -> {
            config.insertJson5("listen/endpoints", illFormattedEndpoints);
        });

        String correctEndpoints = "[\"tcp/8.8.8.8:8\", \"tcp/8.8.8.8:9\"]";
        config.insertJson5("listen/endpoints", correctEndpoints);
        String retrievedEndpoints = config.getJson("listen/endpoints");

        assertTrue(retrievedEndpoints.contains("8.8.8.8"));
    }
}
