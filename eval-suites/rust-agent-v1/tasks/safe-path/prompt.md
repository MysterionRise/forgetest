Harden `safe_join` against absolute paths and lexical traversal. Accept only
normal path components and `.` beneath the supplied root. Reject `..`, root,
and platform prefix components without accessing the filesystem.
