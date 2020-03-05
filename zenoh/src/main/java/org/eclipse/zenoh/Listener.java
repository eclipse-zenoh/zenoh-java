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

import java.util.List;

/**
 * Interface to be implemented for subscriptions (see
 * {@link Workspace#subscribe(Selector, Listener)})
 */
public interface Listener {

    /**
     * The callback operation called for all changes on subscribed paths.
     *
     * @param changes the list of changes.
     */
    public void onChanges(List<Change> changes);
}
