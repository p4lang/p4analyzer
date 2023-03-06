if not exist ".\lib\wasm\" (
    mkdir .\lib\wasm
)

ROBOCOPY ../../crates/p4-analyzer-wasm/pkg/ ./lib/wasm p4_analyzer_wasm.*
ROBOCOPY ../../crates/p4-analyzer-wasm/pkg/ ./lib/wasm *.wasm

exit /b 0
