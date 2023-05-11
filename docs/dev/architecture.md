# Architecture
The P4 Analyzer is to be packaged and deployed both as a native CLI executable, and a WebAssembly based Node.js package.
The native executable will be built for Windows, Linux and MacOS, and configurable for use with any Language Server
Protocol (LSP) client. The WebAssembly/Node.js build will be deployed as part of a Visual Studio Code extension,
thereby negating the need to download any platform specific assets when the extension is first initialized.

This document describes some of the high level architectural pieces that exist in the project, and can be used as a
guide when looking at navigating the codebase for the first time.

## <a name="analyzer-host"></a> The `AnalyzerHost` and `Analyzer`
The `Analyzer` is responsible for ingesting P4 source code and producing a structured model of that P4 code that is
highly optimized for querying. The model maintained by the `Analyzer` is fully resolved and wholly maintained in
memory, and as such, having no direct access to I/O, will be derived from the received LSP notifications directly.
Request and Notification Handlers adapt the LSP messages to one or more calls to the core Analyzer. This design will
allow the core Analyzer to be used outside of a Language Server context and in other tools such as Formatters and
Linters.

Since the `Analyzer` cannot perform any I/O directly, it will rely on a host to provide these (and other) services
on its behalf. The following diagram shows this composition:

![AnalyzerHost and Analyzer composition.](diagrams/analyzer-host.svg)

The `AnalyzerHost` will receive LSP client requests and notifications via the _receive_ port of a `MessageChannel`. It
will also send any responses and server notifications to the _send_ port of the same channel if a response is required
for that message.

Since it is possible for the LSP server to make requests on the LSP client (which is managed via the `RequestManager`),
interally, the `AnalyzerHost` will filter responses from requests and notifications before sending them onto additional
internal `MessageChannel`s. Requests and Notifications are processed by a simple finite state machine
(`ProtocolMachine`) that models the LSP. For any given LSP implementation, a server has a lifecycle that is fully
managed by the client. The `ProtocolMachine` simply ensures that the server is in a valid state for a given request,
based on the state transitions that are causal to the previously processed requests and notifications.

A `WorkspaceManager` will manage one or more `Workspace`s (root folders of interest that the LSP client has opened).
Typically, this will represent the _roots_ of projects that have been opened by the client IDE, but it may also include
the _roots_ to library files that should be included in the analysis. During initialization, in which the client
provides these _root_ paths, the `Workspace`s will be iterated over in order to _prime_ the `Analyzer` with the
initial set of source file texts (a process known as indexing). If the contents of a file is modified on disk, then
either the LSP client will send an appropriate notification describing the change; or, if this is unsupported
by LSP clients, a custom File Watching service that runs outside the `AnalyzerHost` will need to do the same.

### Components
The following table describes the core `AnalyzerHost` and `Analyzer` components:

| Component | Description |
| --- | --- |
| `AnalyzerHost` | Provides a runtime environment for an `Analyzer`, utilizing services that are provided by the host process. |
| `ProtocolMachine` | A state machine that models the Language Server Protocol (LSP). |
| `Dispatch` & `DispatchTarget` | Selects a handler that should be invoked for the received message based on the current LSP state. |
| `State` | Encapsulates all of the state required to run the LSP server. |
| `RequestManger` | Provides an `async` way to send a request to the LSP client and `await` a response. |
| `EnumerableFileSystem` | A very simple utility that can be used to enumerate files within a given path and retrieve file contents. |
| `LspEnumerableFileSystem` | Implements `EnumerableFileSystem` by sending apprpriate requests to the LSP client. Only supported in VSCode. |
| `WorkspaceManager` | Manages a `Workspace` for a given path. |
| `Workspace` | Encapsulates a workspace root and manages the files within it.  |
| `LspTracingLayer` | A [Tokio Tracing](https://tracing.rs/tracing) subscriber that collects the structured, event based diagnostic and trace data, that is emitted by the `AnalyzerHost` and `Analyzer`. It sends this data to the LSP client via a notification. |
| `Analyzer` | The core P4 Analyzer. |

## <a name="native-hosting"></a> Native Executable Hosting
The following diagram extends the previous `AnalyzerHost` and `Analyzer` composition diagram in relation to how they
will be utilized inside a native executable configuration. The native executable will provide a server command
(`LSPServerCommand`) which simply hosts a new `AnalyzerHost` instance alongside a `Driver`:

![](diagrams/native-analyzer-host.svg)

The `Driver` manages two `MessageChannel`s, one for `'stdin'`, and one for `'stdout'`. A `Driver` can be configured
with a given _driver type_ which is responsible for receiving requests and notifications and sending responses over the
transport it supports. Currently we provide the typical STDIN/STDOUT transport, and have a `Console` driver to manage
it.

A receiver thread will read
buffered data from `stdin` and write it to the _send_ port of the `stdin_channel`. Inversely, a send thread will
receive `Message`s from the _receive_ port of the `stdout_channel`, and write it directly to `stdout`. The opposite
ports of `stdin_channel` and `stdout_channel` are then presented to the `AnalyzerHost`, which as we saw previously,
uses them to receive requests and send responses.

Once started, `Console` is simply responsible for reading and writing JSON-RPC messages to the `MessageChannel`
that is supplied to `AnalyzerHost`.

A `RollingFileTrace` is an additional [Tokio Tracing](https://tracing.rs/tracing) subscriber that is used in native
deployments to capture the structured event and trace logs and writes them to a trace file.
