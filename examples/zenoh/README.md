# Zenoh Java examples

## Build instructions

   ```bash
   mvn package
   ```

## Start instructions
   
   The jar file produces by the Maven build is standalone (i.e. it contains all the required dependencies).
   You can run any example Java class with such command:

   ```bash
   java -cp target/zenoh-examples-<version>.jar <classname> <args>
   ```

   Each example accepts the -h or --help option that provides a description of its arguments and their default values.

## Examples description

### ZAddStorage

   Add a storage in the Zenoh router it's connected to.

   Usage:
   ```bash
   java -cp target/zenoh-examples-<version>.jar ZAddStorage [--selector SELECTOR] [--id ID] [--locator LOCATOR]
   ```

   Note that his example doesn't specify the Backend that Zenoh has to use for storage creation.  
   Therefore, Zenoh will automatically select the memory backend, meaning the storage will be in memory
   (i.e. not persistent).

### ZPut

   Put a key/value into Zenoh.  
   The key/value will be stored by all the storages with a selector that matches the key.
   It will also be received by all the matching subscribers (see [ZSub](#ZSub) below).  
   Note that if no storage and no subscriber are matching the key, the key/value will be dropped.
   Therefore, you probably should run [ZAddStorage](#ZAddStorage) and/or [ZSub](#ZSub) before ZPut.

   Usage:
   ```bash
   java -cp target/zenoh-examples-<version>.jar ZPut [--path PATH] [--locator LOCATOR] [--msg MSG]
   ```

### ZPutFloat

   Put a key/value into Zenoh where the value is a float.
   The key/value will be stored by all the storages with a selector that matches the key.
   It will also be received by all the matching subscribers (see [ZSub](#ZSub) below).  
   Note that if no storage and no subscriber are matching the key, the key/value will be dropped.
   Therefore, you probably should run [ZAddStorage](#ZAddStorage) and/or [ZSub](#ZSub) before ZPutFloat.

   Usage:
   ```bash
   java -cp target/zenoh-examples-<version>.jar ZPutFloat [-l=<locator>] [-p=<path>]
   ```

### ZGet

   Get a list of keys/values from Zenoh.  
   The values will be retrieved from the Storages containing paths that match the specified selector.  
   The Eval functions (see [ZEval](#ZEval) below) registered with a path matching the selector
   will also be triggered.

   Usage:
   ```bash
   java -cp target/zenoh-examples-<version>.jar ZGet [--selector SELECTOR] [--locator LOCATOR]
   ```

### ZRemove

   Remove a key and its associated value from Zenoh.  
   Any storage that store the key/value will drop it.  
   The subscribers with a selector matching the key will also receive a notification of this removal.

   Usage:
   ```bash
   java -cp target/zenoh-examples-<version>.jar ZRemove [--path PATH] [--locator LOCATOR]
   ```

### ZSub

   Register a subscriber with a selector.  
   The subscriber will be notified of each put/remove made on any path matching the selector,
   and will print this notification.

   Usage:
   ```bash
   java -cp target/zenoh-examples-<version>.jar ZSub [--selector SELECTOR] [--locator LOCATOR]
   ```

### ZEval

   Register an evaluation function with a path.  
   This evaluation function will be triggered by each call to a get operation on Zenoh 
   with a selector that matches the path. In this example, the function returns a string value.
   See the code for more details.

   Usage:
   ```bash
   java -cp target/zenoh-examples-<version>.jar ZEval [--path PATH] [--locator LOCATOR]
   ```

### ZPutThr & ZSubThr

   Pub/Sub throughput test.
   This example allows to perform throughput measurements between a pubisher performing
   put operations and a subscriber receiving notifications of those put.
   Note that you can run this example with or without any storage.

   Subscriber usage:
   ```bash
   java -cp target/zenoh-examples-<version>.jar ZSubThr [--path PATH] [--locator LOCATOR]
   ```

   Publisher usage:
   ```bash
   java -cp target/zenoh-examples-<version>.jar ZPutThr [--size SIZE] [--locator LOCATOR] [--path PATH] [-b=BUFFER_KIND]
   ```

   where the BUFFER_KIND option specifies the way to allocate the java.util.ByteBuffer payload that will be put into Zenoh. The possible values are:
   - **DIRECT**: use a direct ByteBuffer. This is the default value.
   - **NON_DIRECT**: use a non-direct ByteBuffer (created via ByteBuffer.allocate())  
   - **WRAPPED**: use a wrapped ByteBuffer (created via ByteBuffer.wrap())  
