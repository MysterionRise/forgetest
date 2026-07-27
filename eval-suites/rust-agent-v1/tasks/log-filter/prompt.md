Implement `parse_filter` for comma-separated directives. A bare level sets the
default, and `target=level` sets a target override. Supported levels are
`error`, `warn`, `info`, `debug`, and `trace`, case-insensitively. Reject empty
directives, unknown levels, and empty target names. Later directives override
earlier directives.
