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
package org.eclipse.zenoh.net;

import org.eclipse.zenoh.core.ZException;
import org.eclipse.zenoh.swig.zn_temporal_property_t;

/**
 * Subscription mode (used in
 * {@link Session#declareSubscriber(String, SubMode, SubscriberCallback)}).
 */
public class SubMode extends org.eclipse.zenoh.swig.zn_sub_mode_t {

    /**
     * The subscription mode kind.
     */
    public enum Kind {
        ZN_PUSH_MODE((short) 1), ZN_PULL_MODE((short) 2), ZN_PERIODIC_PUSH_MODE((short) 3),
        ZN_PERIODIC_PULL_MODE((short) 4);

        private short numVal;

        Kind(short numVal) {
            this.numVal = numVal;
        }

        public static Kind fromInt(short numVal) throws ZException {
            if (numVal == ZN_PUSH_MODE.value()) {
                return ZN_PUSH_MODE;
            } else if (numVal == ZN_PULL_MODE.value()) {
                return ZN_PULL_MODE;
            } else if (numVal == ZN_PERIODIC_PUSH_MODE.value()) {
                return ZN_PERIODIC_PUSH_MODE;
            } else if (numVal == ZN_PERIODIC_PULL_MODE.value()) {
                return ZN_PERIODIC_PULL_MODE;
            } else {
                throw new ZException("INTERNAL ERROR: cannot create SubMode.Kind from int: " + numVal);
            }
        }

        public short value() {
            return numVal;
        }
    }

    private static SubMode PUSH_MODE = new SubMode(Kind.ZN_PUSH_MODE);
    private static SubMode PULL_MODE = new SubMode(Kind.ZN_PULL_MODE);

    private SubMode(Kind kind) {
        super();
        setKind(kind.value());
    }

    private SubMode(Kind kind, int origin, int period, int duration) {
        super();
        setKind(kind.value());
        zn_temporal_property_t tprop = new zn_temporal_property_t();
        tprop.setOrigin(origin);
        tprop.setPeriod(period);
        tprop.setDuration(duration);
        setTprop(tprop);
    }

    /**
     * @return the push subscription mode.
     */
    public static SubMode push() {
        return PUSH_MODE;
    }

    /**
     * @return the pull subscription mode.
     */
    public static SubMode pull() {
        return PULL_MODE;
    }

    /**
     * @return a periodic push subscription mode with the specified temporal
     *         properties.
     */
    public static SubMode periodicPush(int origin, int period, int duration) {
        return new SubMode(Kind.ZN_PERIODIC_PUSH_MODE, origin, period, duration);
    }

    /**
     * @return a periodic pull subscription mode with the specified temporal
     *         properties.
     */
    public static SubMode periodicPull(int origin, int period, int duration) {
        return new SubMode(Kind.ZN_PERIODIC_PULL_MODE, origin, period, duration);
    }
}
