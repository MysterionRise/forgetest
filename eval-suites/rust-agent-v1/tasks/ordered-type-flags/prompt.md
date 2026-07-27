This reduced fixture preserves the ordered override behavior from ripgrep
commit `3b9f44671e3a60ef0cc9b4b3a1e61f59b36f5342`. Fix `selected_types` so
include and exclude operations are applied in command-line order. A later
operation for the same type must override an earlier one.
