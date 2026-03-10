# Architecture

Rustalink is built on a modern, asynchronous foundation using Tokio, ensuring low latency and high concurrency.

## Core Components

- **REST API:** Powered by Axum for lightning-fast JSON processing.
- **WebSocket Manager:** Real-time state synchronization and command routing.
- **Audio Pipeline:** Efficient raw packet decoding, routing, and encoding.
- **Filter Engine:** 32-bit floating-point DSP engine for studio-grade effects.

## Data Flow

```mermaid
graph LR
    Client --> API[REST / WS]
    API --> Pipeline
    Pipeline --> Filters
    Filters --> Discord[Discord Voice]
```
