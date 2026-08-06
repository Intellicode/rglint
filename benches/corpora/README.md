# Benchmark corpora

These are small, license-clean GraphQL corpora shaped like the public GitHub
and Shopify APIs. They are maintained as first-party benchmark inputs rather
than copied service introspection dumps, so benchmark runs remain reproducible
and do not depend on network access or third-party redistribution terms.

The names identify the API shape exercised by each corpus. Each file is
deliberately large enough to exercise schema loading, CST traversal, typed
operation loading, sibling indexing, and the recommended rule set while still
keeping `cargo bench --no-run` and local benchmark runs practical.
