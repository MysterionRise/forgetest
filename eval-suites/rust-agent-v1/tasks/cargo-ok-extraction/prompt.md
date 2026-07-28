This reduced fixture preserves the core behavior of Cargo's
CVE-2022-36113 fix in commit `97b80919e404b0768ea31ae329c3b4da54bed05a`.
Do not extract any archive entry whose file name is `.cargo-ok`, and configure
the post-extraction marker to use create-new semantics rather than overwriting
an existing path.
