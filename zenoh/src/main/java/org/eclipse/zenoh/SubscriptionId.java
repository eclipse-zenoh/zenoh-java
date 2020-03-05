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
package org.eclipse.zenoh;

import org.eclipse.zenoh.net.Subscriber;

/**
 * The identifier of a subscription.
 * 
 * @see Workspace#subscribe(Selector, Listener)
 * @see Workspace#unsubscribe(SubscriptionId)
 */
public final class SubscriptionId {

    private Subscriber sub;

    protected SubscriptionId(Subscriber sub) {
        this.sub = sub;
    }

    protected Subscriber getZSubscriber() {
        return sub;
    }
}
