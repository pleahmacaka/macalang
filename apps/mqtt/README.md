# mqtt: a broker and client, in Maca

A working **MQTT 3.1.1 broker and client** written in Maca over a C FFI
(`import c "mqtt.h"`, resolved by the binding glue in `maca-runtime`). Both
compile to native binaries and link libc sockets + pthreads (no external
dependency), so they build in the static-musl path like any other Maca program.

## Broker (`broker.maca`)

A threaded broker that accepts `CONNECT`/`SUBSCRIBE`/`PUBLISH`, serves many
concurrent clients, and routes on topic filters including the `+` (single
level) and `#` (multi level) wildcards.

```sh
maca run apps/mqtt/broker.maca      # listens on :1883
```

## Client (`client.maca`)

A CLI that subscribes or publishes, dispatching on `main(args: str[])` with a
list `match` (`"sub", ..rest` / `"pub", ..rest`):

```sh
maca build apps/mqtt/client.maca -o mqttc
./mqttc sub                 # subscribe to test/# and print the next message
./mqttc pub hello world     # publish "hello world" to test/x
```

The foreign functions (`mqtt_broker_run`, `mqtt_connect`, `mqtt_subscribe`,
`mqtt_publish`, `mqtt_receive`, `mqtt_disconnect`) are declared bodyless in the
`.maca` source and implemented in `MQTT_GLUE` (`crates/runtime`).
