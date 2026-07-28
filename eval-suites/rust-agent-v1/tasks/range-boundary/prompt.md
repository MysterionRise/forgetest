Fix `plan_chunks` so every tuple uses an exclusive end offset and the chunks
cover exactly `0..total`. Preserve the public API and behavior for zero-sized
inputs.
