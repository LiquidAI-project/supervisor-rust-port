What is SENTRY_DSN?



-- Connecting supervisor to orchestrator --

1. Supervisor initializes, gets its own ip-address, port, etc ???
2. Once supervisor is ready (TODO: Check that there is a readiness check), broadcast mDns message to
orchestrator.
    * Orchestrator address is read from ???
    * POST http://<orchestrator_address>:<orchestrator_port>/file/device/discovery/register
3. Orchestrator fetches device description
    * Address ???
    * Returns ???

-- API for running WASM-modules --

???

-- Communication between orchestrator and supervisor --

???

