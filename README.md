# Papers

A document manager based on [paperless-ngx](https://docs.paperless-ngx.com/), optimised
for a memory-constrained environment. In order to not sacrifice performance, the
entire back-end is written in asynchronous Rust (tokio) instead of Python.

## System Design Choices

- In order to have memory guarantees, the **index** needed to query data is capped at 50 MB.
  Any over-flow is stored on disk and is merged with the index ad-hoc at query
  time (IN DEVELOPMENT)
- Keep a single blocking thread for CPU-intensive tasks (e.g. indexing,
  generating PDF thumbnails etc.). Most of the system activity is composed of I/O bound tasks
  are managed by the Tokio runtime to keep the system responsive.
