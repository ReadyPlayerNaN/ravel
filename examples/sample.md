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
| Images, audio and video | Supported |

## Links and images

[Example link](https://example.org) opens in your browser.
[Back to the lists](#lists-and-quotes) navigates inside this document.

![Ravel diagram](media/ravel.svg "Local SVG image")

## Audio and video

Press Play to try these small local samples. Nothing plays automatically.

![Quiet test tone](media/tone.ogg)

![Animated test pattern](media/demo.webm)

## Math and footnotes

Inline math: $E = mc^2$ and $\frac{a}{b}$.

$$\int_0^1 x^2\,dx = \frac{1}{3}$$

A note can be referenced more than once[^note], with a return link for each reference[^note].

[^note]: Footnotes support **formatting**, links, and multiple paragraphs.

    Task checkbox changes are temporary; the Markdown file remains unchanged.

---

HTML is literal text, never executed:

<script>not executed</script>
