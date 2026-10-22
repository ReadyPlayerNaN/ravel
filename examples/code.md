# Highlighted code blocks

Inline `const plain = true;` stays inline. Language-tagged fences are highlighted
without running the code or changing the source.

```javascript
// JavaScript
const greeting = "Hello, Markdown!";
function greet(name) {
    return `${greeting} ${name}`;
}
window.codeExecuted = true;
```

```JS extra-info
const alias = 42;
```

```json
{"message": "hello", "count": 42, "enabled": true}
```

```rust
fn main() {
    println!("Hello, Rust!");
}
```

```html
<script>window.codeExecuted = true;</script>
<img src="https://example.invalid/must-not-load.png" onerror="window.codeExecuted = true">
```

Unknown languages, untagged fences, and indented code remain plain:

```not-a-language
const unknown = true;
```

```
const untagged = true;
```

    const indented = true;
