# JSON inside Markdown

The same native structured viewer appears inside each tagged JSON block. Use the
header button to switch to the original raw JSON without losing expanded nodes.

```json
{
  "name": "Ravel",
  "users": [{"name": "Alice", "active": true}, {"name": "Bob", "active": false}],
  "count": 123456789012345678901234567890,
  "a/b": {"~key": "Unicode: 日本語"}
}
```

[An ordinary link](https://example.org) still works between native blocks.

```JSON
[{"second": true}, null, 42]
```

Invalid JSON remains raw and does not prevent reading the rest of the document:

```json
{"unfinished":
```

Untagged JSON is still plain code:

```
{"plain": true}
```

## Following content

- [ ] Tasks remain temporary.

The original Markdown and raw JSON whitespace are never changed.
