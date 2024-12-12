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

import com.google.common.reflect.TypeToken;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import static io.zenoh.ext.ZSerializerKt.zSerialize;

public class ZBytes {

    public static void main(String[] args) {
//        var variable = 10f;
//        var variable = Map.of("a", Map.of("c", "d"));
        var variable = new HashMap<String, String>();
        zSerialize(variable, new TypeToken<>() {});
//        ZSerializer.zSerialize(map, new TypeToken<>() {});
    }
}
