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
package org.eclipse.zenoh.core;

import org.eclipse.zenoh.swig.zenohcConstants;
import java.util.HashMap;

/**
 * An Exception raised by Zenoh.
 */
public class ZException extends Exception {

    public static final Integer Z_NO_ERROR_CODE = 0x00;

    public static final Integer Z_VLE_PARSE_ERROR = zenohcConstants.Z_VLE_PARSE_ERROR;
    public static final Integer Z_ARRAY_PARSE_ERROR = zenohcConstants.Z_ARRAY_PARSE_ERROR;
    public static final Integer Z_STRING_PARSE_ERROR = zenohcConstants.Z_STRING_PARSE_ERROR;

    public static final Integer ZN_PROPERTY_PARSE_ERROR = zenohcConstants.ZN_PROPERTY_PARSE_ERROR;
    public static final Integer ZN_PROPERTIES_PARSE_ERROR = zenohcConstants.ZN_PROPERTIES_PARSE_ERROR;
    public static final Integer ZN_MESSAGE_PARSE_ERROR = zenohcConstants.ZN_MESSAGE_PARSE_ERROR;
    public static final Integer ZN_INSUFFICIENT_IOBUF_SIZE = zenohcConstants.ZN_INSUFFICIENT_IOBUF_SIZE;
    public static final Integer ZN_IO_ERROR = zenohcConstants.ZN_IO_ERROR;
    public static final Integer ZN_RESOURCE_DECL_ERROR = zenohcConstants.ZN_RESOURCE_DECL_ERROR;
    public static final Integer ZN_PAYLOAD_HEADER_PARSE_ERROR = zenohcConstants.ZN_PAYLOAD_HEADER_PARSE_ERROR;
    public static final Integer ZN_TX_CONNECTION_ERROR = zenohcConstants.ZN_TX_CONNECTION_ERROR;
    public static final Integer ZN_INVALID_ADDRESS_ERROR = zenohcConstants.ZN_INVALID_ADDRESS_ERROR;
    public static final Integer ZN_FAILED_TO_OPEN_SESSION = zenohcConstants.ZN_FAILED_TO_OPEN_SESSION;
    public static final Integer ZN_UNEXPECTED_MESSAGE = zenohcConstants.ZN_UNEXPECTED_MESSAGE;

    private static final java.util.Map<Integer, String> errorCodeToString = new HashMap<Integer, String>();

    static {
        errorCodeToString.put(Z_ARRAY_PARSE_ERROR, "Z_ARRAY_PARSE_ERROR");
        errorCodeToString.put(Z_STRING_PARSE_ERROR, "Z_STRING_PARSE_ERROR");
        errorCodeToString.put(ZN_PROPERTY_PARSE_ERROR, "ZN_PROPERTY_PARSE_ERROR");
        errorCodeToString.put(ZN_PROPERTIES_PARSE_ERROR, "ZN_PROPERTIES_PARSE_ERROR");
        errorCodeToString.put(ZN_MESSAGE_PARSE_ERROR, "ZN_MESSAGE_PARSE_ERROR");
        errorCodeToString.put(ZN_INSUFFICIENT_IOBUF_SIZE, "ZN_INSUFFICIENT_IOBUF_SIZE");
        errorCodeToString.put(ZN_IO_ERROR, "ZN_IO_ERROR");
        errorCodeToString.put(ZN_RESOURCE_DECL_ERROR, "ZN_RESOURCE_DECL_ERROR");
        errorCodeToString.put(ZN_PAYLOAD_HEADER_PARSE_ERROR, "ZN_PAYLOAD_HEADER_PARSE_ERROR");
        errorCodeToString.put(ZN_TX_CONNECTION_ERROR, "ZN_TX_CONNECTION_ERROR");
        errorCodeToString.put(ZN_INVALID_ADDRESS_ERROR, "ZN_INVALID_ADDRESS_ERROR");
        errorCodeToString.put(ZN_FAILED_TO_OPEN_SESSION, "ZN_FAILED_TO_OPEN_SESSION");
        errorCodeToString.put(ZN_UNEXPECTED_MESSAGE, "ZN_UNEXPECTED_MESSAGE");
    }

    private int errorCode;

    private static final long serialVersionUID = 402535504102337839L;

    public ZException(String message) {
        this(message, Z_NO_ERROR_CODE);
    }

    public ZException(String message, Throwable cause) {
        this(message, Z_NO_ERROR_CODE, cause);
    }

    public ZException(String message, int errorCode) {
        super(message);
        this.errorCode = errorCode;
    }

    public ZException(String message, int errorCode, Throwable cause) {
        super(message, cause);
        this.errorCode = errorCode;
    }

    public int getErrorCode() {
        return errorCode;
    }

    public String getErrorCodeName() {
        String name = errorCodeToString.get(errorCode);
        return (name != null ? name : "UNKOWN_ERROR_CODE(" + errorCode + ")");
    }

    public String toString() {
        if (errorCode != Z_NO_ERROR_CODE) {
            return super.toString() + " (error code:" + getErrorCodeName() + ")";
        } else {
            return super.toString();
        }
    }
}
