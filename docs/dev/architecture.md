# Architecture
The P4 Analyzer is to be packaged and deployed both as a native CLI executable, and a WebAssembly based Node.js package. The native executable will be built for Windows, Linux and MacOS, and configurable for use with any Language Server Protocol (LSP) client. The WebAssembly/Node.js build can be deployed intrisically as part of a Visual Studio Code extension, thereby negating the need to download any platform specific assets when the extension is first initialized.

This document describes some of the high level architectural decisions that have been made for the project, and can be used as a guide when looking at first navigating the codebase.

## <a name="analyzer-host"></a> The `AnalyzerHost` and `Analyzer`
The `Analyzer` is responsible for ingesting P4 source code and producing a structured model of that P4 code that is highly optimized for querying. More specifically, P4 source code will be ingested by the `Analyzer` through a series of (\<_DocumentUri_\>, \<_Range_\>, `String`) tuples that represent both the initial contents, and then the subsequent changes or edits that are made to those source code files. The model maintained by the `Analyzer` is fully resolved and wholly maintained in memory, and as such, being derived from _change deltas_, will not be derived from any direct  I/O.

Since the `Analyzer` cannot perform any I/O directly, it will rely on a host to provide these (and other) services on its behalf. The following diagram shows this composition:

![AnalyzerHost and Analyzer composition.](diagrams/analyzer-host.svg)

The `AnalyzerHost` will receive LSP client requests and notifications via the _receive_ port of a `MessageChannel`. It will also send any responses and server notifications to the _send_ port of the same channel if a response is required.

Internally, the `AnalyzerHost` uses a simple finite state machine (`ProtocolMachine`) that models the LSP. For any given LSP implementation, a server has a lifecycle that is fully managed by the client. The `ProtocolMachine` simply ensures that the server is in a valid state for a given request, based on the state transitions that are causal to the previously processed requests.

> **ℹ Analyzer and P4 Source File Process**
In the above diagram, `Analyzer` is receiving source file changes through a dedicated channel (`source_change_channel`). This design is not concrete and it may change in the future. For example, if source file changes can be processed quickly enough, then changes may be propagated to the `Analyzer` using a simple synchronous function rather than a channel.

A `P4Workspace` will represent a _workspace root_, a folder of interest that the LSP client has informed the LSP server of. Typically, this will represent the _roots_ of projects that have been opened by the client IDE, but it may also include the _roots_ to library files that should be included in the analysis. During initialization, in which the client provides these _root_ paths, the `P4Workspace`s will be iterated over in order to prime the `Analyzer` with the initial set of source file texts. If the contents of a file is modified on disk, then the `P4Workspace` will notify the analyzer host of the external file change.

The decision to forward the external file change to the `Analyzer`, will then depend on whether the server has received a notification from the client that the file has been opened in the IDE. The LSP specification indicates that on receipt of that notification, an LSP server can assume that the client is wholly responsible for the changes made to that file. As such, external file changes should not be forwarded, and instead, only client change notifications until the LSP server receives a notification that the client has closed that file.

### Components
The following table describes the core `AnalyzerHost` and `Analyzer` components:

| Component | Description |
| --- | --- |
| `AnalyzerHost` | Provides a runtime environment for an `Analyzer`, utilizing services that are provided by the host process. |
| `ProtocolMachine` | A state machine that models the Language Server Protocol (LSP). |
| `Analyzer` | The core P4 Analyzer. |
| `P4WorkspaceFactory` | Creates a `P4Workspace` for a given path. |
| `P4Workspace` | Encapsulates a workspace root. The implementation is provided by the host process.  |
| `TracingSubscriber` | Collects the structured, event based diagnostic and trace data that is emitted by the `AnalyzerHost` and `Analyzer`. |

## <a name="native-hosting"></a> Native Executable Hosting
The following diagram extends the previous `AnalyzerHost` and `Analyzer` composition diagram in relation to how they will be utilized inside a native executable configuration. The native executable will provide a server command (`LSPServerCommand`) which simply hosts a new `AnalyzerHost` instance alongside a `ConsoleDriver`:

![](diagrams/native-analyzer-host.svg)

The `ConsoleDriver` manages two `MessageChannel`s, one for `stdin`, and one for `stdout`. A receiver thread will read buffered data from `stdin` and write it to the _send_ port of the `stdin_channel`. Inversely, a send thread will receive `Message`s from the _receive_ port of the `stdout_channel`, and write it directly to `stdout`. The opposite ports of `stdin_channel` and `stdout_channel` are then presented to the `AnalyzerHost`, which as we saw previously, uses them to receive requests and send responses.

Once started, `ConsoleDriver` is simply responsible for reading and writing JSON-RPC messages to the `MessageChannel` that is supplied to `AnalyzerHost`.

A `CommandTracingSubscriber` is used in native deployments to capture the structured event and trace logs. The output of which will be written to a trace file.
