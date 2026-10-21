# Ravel Markdown

A **read-only** formatted document, with *emphasis*, ~~strikethrough~~ and `inline code`.
Unicode works too: café, 日本語, 🦀.

## Lists and quotes

- Open a Markdown file
- Switch between Formatted and Raw Markdown
  - The original source is preserved
- [x] Read without editing
- [ ] Explore another document

3. Ordered lists retain their starting number
4. Nested content stays indented

> Documents remain unchanged. Select text to copy it, or use the header button
> to copy the original Markdown source.

## Code

```rust
fn main() {
    println!("Hello, Markdown!");
}
```

## Table

| Feature | Status |
| --- | --- |
| Formatted text | **Supported** |
| Editing | Not enabled |
| Network resources | Not loaded |

## Links and images

[Example link](https://example.org) is displayed with its destination, not opened.

![Example image](https://example.org/image.png)

---

HTML is literal text, never executed:

<script>not executed</script>
